import type { LogLine } from './stream'

/**
 * What a line turns out to be, once the terminal formatting is read off it.
 *
 * Products write for a terminal: colour codes, their own timestamp, a level
 * and a module, all inside one string. Shown as text, that is forty characters
 * of markup before the first word anybody wanted. Read apart, it is four
 * columns and a filter.
 */
export interface LogRecord {
  at: string
  source: string
  level: Level | null
  /** The module or logger that wrote it, where the line names one. */
  target: string | null
  /** What is left once the rest has been lifted out. */
  text: string
}

export type Level = 'trace' | 'debug' | 'info' | 'warn' | 'error'

/** Least severe first. What "this level and above" is counted against. */
export const LEVELS: Level[] = ['trace', 'debug', 'info', 'warn', 'error']

/**
 * Terminal control sequences, which every colouring logger emits and no
 * browser renders.
 *
 * The escape byte is invisible in HTML, so what reaches the reader is the
 * tail of each sequence as literal text — `[2m`, `[0m`, `[34m` — which on a
 * `tracing` line outweighs the message it decorates.
 */
// The rule exists to catch a control character nobody meant to write. Here
// they are the subject: an escape sequence is what is being matched.
const ANSI =
  // eslint-disable-next-line no-control-regex
  /\u001b\[[0-9;:?]*[ -/]*[@-~]|\u001b\][^\u0007]*(?:\u0007|\u001b\\)|\u001b./g

/**
 * The same sequences with the escape byte already gone.
 *
 * Belt and braces: anything that normalises a log line on the way here — a
 * sidecar, a collector, a JSON encoder — may drop the escape and leave the
 * tail behind, and the tail is the part that is actually visible. A message
 * genuinely containing `[0m` loses it, which is a far smaller loss than a
 * screen nobody can read.
 */
const BARE_SGR = /\[[0-9;]{1,4}m/g

export function stripAnsi(text: string): string {
  return text.replace(ANSI, '').replace(BARE_SGR, '')
}

/**
 * The timestamp a product writes itself.
 *
 * Lifted off because the platform already stamps every line with the moment
 * its container wrote it, and two clocks in one row is a column of noise
 * beside a column of the same thing.
 */
const LEADING_STAMP =
  /^\s*\d{4}[-/]\d{2}[-/]\d{2}[T ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[+-]\d{2}:?\d{2})?\s*/

/**
 * How far into a line a level may sit and still be the line's level.
 *
 * Bounded because "the deployment failed" is a sentence, not a severity, and
 * a search over the whole message finds one in every other paragraph.
 */
const LEVEL_REACH = 40

const NAMED: Record<string, Level> = {
  TRACE: 'trace',
  DEBUG: 'debug',
  INFO: 'info',
  NOTICE: 'info',
  WARNING: 'warn',
  WARN: 'warn',
  ERROR: 'error',
  FATAL: 'error',
  SEVERE: 'error',
  CRIT: 'error',
  ALERT: 'error',
  EMERG: 'error',
}

const LEVEL = /\b(TRACE|DEBUG|INFO|NOTICE|WARNING|WARN|ERROR|FATAL|SEVERE|CRIT|ALERT|EMERG)\b/

/**
 * The severity nginx and its family write, in brackets and in lower case.
 *
 * Matched only in brackets and only at the front, because reading a bare
 * lower-case `info` as a severity would find one in half the sentences a
 * product logs.
 */
const BRACKETED_LEVEL =
  /^\s*\[(trace|debug|info|notice|warning|warn|error|fatal|crit|alert|emerg)\]\s*/

/**
 * The module that wrote the line: `rustls::client::hs` for anything built on
 * `tracing`, `[org.keycloak.services]` for anything on the JVM.
 *
 * Only those two shapes, and only at the front. Guessing more widely would
 * take the first word of a message that never named a module and file it as
 * one.
 */
const TARGET = /^\s*(?:([\w$.]+(?:::[\w$.]+)+)\s*:?|\[([^\]\s]{1,80})\])\s*/

export function toRecord(line: LogLine): LogRecord {
  const clean = stripAnsi(line.message)
  const body = clean.replace(LEADING_STAMP, '')

  const bracketed = body.match(BRACKETED_LEVEL)
  const found = bracketed ?? body.slice(0, LEVEL_REACH).match(LEVEL)
  const level = found ? (NAMED[found[1].toUpperCase()] ?? null) : null
  const rest =
    found && found.index !== undefined
      ? body.slice(0, found.index) + body.slice(found.index + found[0].length)
      : body

  const named = rest.match(TARGET)
  const target = named ? (named[1] ?? named[2]) : null
  const left = named ? rest.slice(named[0].length) : rest

  // Trimmed only where something was lifted out, so the space a removed level
  // left behind goes with it. A line nothing was taken from keeps its
  // indentation: on half a pretty-printed struct, that indentation is the
  // only thing saying which struct it belongs to.
  const lifted = body.length !== clean.length || level !== null || target !== null

  return {
    at: line.at,
    source: line.source,
    level,
    target,
    text: lifted ? left.trim() : left.trimEnd(),
  }
}

/**
 * Whether a record clears the floor the reader set.
 *
 * A line whose level could not be read always shows. Hiding what we failed to
 * parse would make the filter delete evidence, which is the one thing a log
 * view must never do.
 */
export function clears(record: LogRecord, floor: Level): boolean {
  if (record.level === null) return true

  return LEVELS.indexOf(record.level) >= LEVELS.indexOf(floor)
}

/** Whether a record answers what is being looked for. */
export function matches(record: LogRecord, query: string): boolean {
  const wanted = query.trim().toLowerCase()
  if (wanted === '') return true

  return (
    record.text.toLowerCase().includes(wanted) ||
    record.source.toLowerCase().includes(wanted) ||
    (record.target?.toLowerCase().includes(wanted) ?? false)
  )
}

/** The records worth drawing, for a floor and a search. */
export function narrow(records: LogRecord[], floor: Level, query: string): LogRecord[] {
  if (floor === 'trace' && query.trim() === '') return records

  return records.filter((record) => clears(record, floor) && matches(record, query))
}
