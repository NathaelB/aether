import type { LogRecord } from './record'

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

/**
 * What copying or downloading hands over.
 *
 * The line as it was read, not as it arrived: the colour codes are gone, and
 * what is pasted into an issue is what was on the screen.
 */
export function asText(records: LogRecord[], clock: Clock): string {
  return records
    .map((record) =>
      [
        readStamp(record.at, clock),
        record.source,
        record.level?.toUpperCase(),
        record.target,
        record.text,
      ]
        .filter((part) => part !== null && part !== undefined)
        .join(' '),
    )
    .join('\n')
}
