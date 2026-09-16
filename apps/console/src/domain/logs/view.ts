import type { LogLine } from './stream'

/**
 * Which time a stamp is read in.
 *
 * Both are the same instant. Somebody correlating a log with their own
 * afternoon wants their clock; somebody correlating it with a trace or with a
 * colleague in another country wants the instance's.
 */
export type Clock = 'browser' | 'utc'

/**
 * How many colours sources are told apart by.
 *
 * Small on purpose. A deployment runs two or three containers, and a palette
 * wide enough that no two ever collide would be a palette of colours nobody
 * can tell apart.
 */
export const SOURCE_TONES = 6

/**
 * A stable tone for a container's name.
 *
 * Derived from the name rather than from the order sources appear, so the
 * colour a container had before a reconnect is the colour it has after one.
 */
export function toneFor(source: string): number {
  let hash = 0

  for (let index = 0; index < source.length; index += 1) {
    hash = (hash * 31 + source.charCodeAt(index)) | 0
  }

  return Math.abs(hash) % SOURCE_TONES
}

/**
 * Whether a line answers what is being looked for.
 *
 * Over the message and the source both: "the postgres one" is as much a way
 * of narrowing a screen as a word in the text is. Case is ignored, because a
 * log is not written for grepping.
 */
export function matches(line: LogLine, query: string): boolean {
  const wanted = query.trim().toLowerCase()
  if (wanted === '') return true

  return (
    line.message.toLowerCase().includes(wanted) || line.source.toLowerCase().includes(wanted)
  )
}

/** The lines worth drawing, for what is being looked for. */
export function narrow(lines: LogLine[], query: string): LogLine[] {
  if (query.trim() === '') return lines

  return lines.filter((line) => matches(line, query))
}

/**
 * How a stamp reads.
 *
 * Seconds included, milliseconds not: two lines a millisecond apart are
 * ordered by where they sit, and the extra digits cost a column that a long
 * message needs more.
 */
export function readStamp(at: string, clock: Clock): string {
  const moment = new Date(at)
  if (Number.isNaN(moment.getTime())) return at

  if (clock === 'utc') return moment.toISOString().slice(11, 19)

  return moment.toLocaleTimeString([], { hour12: false })
}

/**
 * Whether a scroller is at its end, within a line or two.
 *
 * Not exactly at the end: following the tail smoothly leaves the scroller a
 * few pixels short often enough that an exact test would read as the reader
 * having scrolled up, and stop following on its own.
 */
export function atTheEnd(scroller: { scrollTop: number; scrollHeight: number; clientHeight: number }): boolean {
  return scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 48
}

/** What copying or downloading hands over. */
export function asText(lines: LogLine[], clock: Clock): string {
  return lines.map((line) => `${readStamp(line.at, clock)} ${line.source} ${line.message}`).join('\n')
}
