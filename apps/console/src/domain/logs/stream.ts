export interface LogLine {
  at: string
  source: string
  message: string
}

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

/** Keeps the newest lines and drops the oldest. */
export function keepRecent(held: LogLine[], arriving: LogLine[]): LogLine[] {
  const all = [...held, ...arriving]
  return all.length <= KEPT_LINES ? all : all.slice(all.length - KEPT_LINES)
}

export interface StreamOptions {
  url: string
  token: string
  signal: AbortSignal
  onLines: (lines: LogLine[]) => void
}

/**
 * Reads the server sent stream.
 *
 * Not `EventSource`, which cannot carry an Authorization header and would
 * force the token into the query string, where it would land in every proxy
 * and access log between here and the control plane.
 */
export async function readLogStream({ url, token, signal, onLines }: StreamOptions): Promise<void> {
  const response = await fetch(url, {
    headers: { Authorization: `Bearer ${token}`, Accept: 'text/event-stream' },
    signal,
  })

  if (!response.ok || !response.body) {
    throw new Error(`HTTP ${response.status}`)
  }

  const decoder = new TextDecoder()
  const reader = response.body.getReader()
  let buffer = ''

  while (true) {
    const { done, value } = await reader.read()
    if (done) return

    buffer += decoder.decode(value, { stream: true })
    const { frames, rest } = readFrames(buffer)
    buffer = rest

    const lines = frames.map(toLogLine).filter((line): line is LogLine => line !== null)
    if (lines.length > 0) onLines(lines)
  }
}
