export interface LogLine {
  at: string
  source: string
  message: string
}

/**
 * Why a session is over, as the control plane says it.
 *
 * Read rather than assumed. A stream that simply stops looks the same as a
 * dropped connection, and the two call for opposite answers: one is worth
 * reopening, the other is worth saying out loud.
 */
export type SessionEnd = 'finished' | 'unreadable' | 'silent'

/** How a read finished. */
export type StreamOutcome =
  | { kind: 'ended'; reason: SessionEnd }
  /** The connection went away without the server saying anything. */
  | { kind: 'dropped' }

/**
 * How far back a read may reach. The server refuses anything longer, so these
 * are offered rather than a free field that produces a rejection.
 */
export const WINDOWS = [
  { minutes: 5, label: 'Last 5 minutes' },
  { minutes: 15, label: 'Last 15 minutes' },
  { minutes: 60, label: 'Last hour' },
] as const

/**
 * How many lines are kept on screen.
 *
 * A busy instance produces more in a minute than anyone reads, and holding
 * all of them turns a log view into a memory leak with a scrollbar.
 */
export const KEPT_LINES = 2000

interface Frame {
  event: string
  data: string
}

/**
 * Splits whatever has arrived so far into whole events, keeping the
 * incomplete tail.
 *
 * A chunk boundary lands in the middle of a line often enough that parsing
 * each chunk on its own drops roughly one event per read.
 */
export function readFrames(buffer: string): { frames: Frame[]; rest: string } {
  const parts = buffer.split('\n\n')
  const rest = parts.pop() ?? ''
  const frames: Frame[] = []

  for (const part of parts) {
    let event = 'message'
    const data: string[] = []

    for (const line of part.split('\n')) {
      if (line.startsWith('event:')) event = line.slice(6).trim()
      else if (line.startsWith('data:')) data.push(line.slice(5).trim())
    }

    if (data.length > 0) frames.push({ event, data: data.join('\n') })
  }

  return { frames, rest }
}

const ENDS: SessionEnd[] = ['finished', 'unreadable', 'silent']

/** The reason carried by an `ended` frame, if it is one. */
export function toSessionEnd(frame: Frame): SessionEnd | null {
  if (frame.event !== 'ended') return null

  try {
    const reason: unknown = JSON.parse(frame.data)?.reason

    // An end we do not recognise is still an end. Treating it as no end at
    // all would leave the screen waiting on a session that is over.
    return ENDS.includes(reason as SessionEnd) ? (reason as SessionEnd) : 'finished'
  } catch {
    return 'finished'
  }
}

export function toLogLine(frame: Frame): LogLine | null {
  if (frame.event !== 'line') return null

  try {
    const parsed = JSON.parse(frame.data)
    if (typeof parsed?.message !== 'string') return null

    return {
      at: typeof parsed.at === 'string' ? parsed.at : new Date().toISOString(),
      source: typeof parsed.source === 'string' ? parsed.source : 'unknown',
      message: parsed.message,
    }
  } catch {
    // A frame that will not parse is one line lost, not a reason to tear the
    // stream down and lose the rest.
    return null
  }
}

function fingerprint(line: LogLine): string {
  return `${line.at}\u0000${line.source}\u0000${line.message}`
}

/**
 * Drops what the previous session already showed.
 *
 * Reopening asks for the same window, so the first lines to arrive are ones
 * already on screen. No line carries an identifier, so sameness is the stamp,
 * the source and the text together -- and only up to the last line held.
 * Past that, two identical lines are two things that happened, and dropping
 * the second would be the log view lying in the other direction.
 */
export function dropOverlap(held: LogLine[], arriving: LogLine[]): LogLine[] {
  const last = held[held.length - 1]
  if (last === undefined) return arriving

  // Compared as instants rather than as text: the stamps carry fractional
  // seconds only when they have any, and "…:01.500Z" sorts before "…:01Z".
  const frontier = Date.parse(last.at)
  const seen = new Set(held.map(fingerprint))

  return arriving.filter(
    (line) => Date.parse(line.at) > frontier || !seen.has(fingerprint(line)),
  )
}

/**
 * How long to wait before reopening, after this many failed attempts.
 *
 * The first reopen is immediate: a session that reached its ceiling is the
 * ordinary case, and making somebody wait a second for it would be a pause
 * they did not ask for. Waiting only grows when reopening keeps failing.
 */
export function backoffMs(attempt: number): number {
  if (attempt <= 1) return 0

  return Math.min(1000 * 2 ** (attempt - 2), 15_000)
}

/**
 * Why this session will not be reopened, or `null` when it will be.
 *
 * Almost every ending is worth reopening: a ceiling reached, a data plane
 * that went quiet, a connection that dropped. Pods that could never be read
 * are the exception -- another session would fail the same way, and retrying
 * for ever would bury the one fact worth showing.
 */
export function whyNotReopened(outcome: StreamOutcome): string | null {
  if (outcome.kind === 'ended' && outcome.reason === 'unreadable') {
    return 'This deployment has no pods that can be read. There is nothing to follow until it is running again.'
  }

  return null
}

/** What the screen says about a read the control plane would not allow. */
export function whyItWasRefused(status: number): string {
  if (status === 403) return 'You may not read this deployment’s logs.'
  if (status === 404) return 'This deployment no longer exists.'

  return `The control plane refused the read (HTTP ${status}).`
}

/** Keeps the newest lines and drops the oldest. */
export function keepRecent(held: LogLine[], arriving: LogLine[]): LogLine[] {
  const all = [...held, ...arriving]
  return all.length <= KEPT_LINES ? all : all.slice(all.length - KEPT_LINES)
}

/**
 * The control plane refused the read.
 *
 * Its own error because the answer is different: a refusal will be refused
 * again, so reopening would spin against a door that is closed on purpose.
 */
export class StreamRefused extends Error {
  constructor(readonly status: number) {
    super(`HTTP ${status}`)
    this.name = 'StreamRefused'
  }
}

export interface StreamOptions {
  url: string
  token: string
  signal: AbortSignal
  onLines: (lines: LogLine[]) => void
  /** Called once the control plane has accepted the read. */
  onOpen?: () => void
}

/**
 * Reads the server sent stream.
 *
 * Not `EventSource`, which cannot carry an Authorization header and would
 * force the token into the query string, where it would land in every proxy
 * and access log between here and the control plane.
 */
export async function readLogStream({
  url,
  token,
  signal,
  onLines,
  onOpen,
}: StreamOptions): Promise<StreamOutcome> {
  const response = await fetch(url, {
    headers: { Authorization: `Bearer ${token}`, Accept: 'text/event-stream' },
    signal,
  })

  if (!response.ok || !response.body) {
    throw new StreamRefused(response.status)
  }

  onOpen?.()

  const decoder = new TextDecoder()
  const reader = response.body.getReader()
  let buffer = ''

  while (true) {
    const { done, value } = await reader.read()
    // Reaching here means the connection went away without the server having
    // said why. Returning it as its own outcome is what keeps that from being
    // read as a session that finished.
    if (done) return { kind: 'dropped' }

    buffer += decoder.decode(value, { stream: true })
    const { frames, rest } = readFrames(buffer)
    buffer = rest

    const lines = frames.map(toLogLine).filter((line): line is LogLine => line !== null)
    if (lines.length > 0) onLines(lines)

    for (const frame of frames) {
      const reason = toSessionEnd(frame)
      // Nothing follows an ending, so the lines above were the last of it and
      // have already been handed over.
      if (reason !== null) return { kind: 'ended', reason }
    }
  }
}
