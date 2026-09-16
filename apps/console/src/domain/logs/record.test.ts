import { describe, expect, it } from 'vitest'
import type { LogLine } from './stream'
import { clears, narrow, stripAnsi, toRecord, type LogRecord } from './record'

const ESC = '\u001b'

function line(message: string, source = 'ferriskey-api'): LogLine {
  return { at: '2026-09-16T07:34:03.166840Z', source, message }
}

/** One real line, as `tracing` writes it with colour on. */
const TRACING =
  `${ESC}[2m2026-09-16T07:34:03.166840Z${ESC}[0m ${ESC}[34mDEBUG${ESC}[0m ` +
  `${ESC}[2mrustls::client::hs${ESC}[0m${ESC}[2m:${ESC}[0m Using ciphersuite TLS13_AES_256_GCM_SHA384`

describe('stripAnsi', () => {
  it('takes the colour codes out', () => {
    expect(stripAnsi(`${ESC}[34mDEBUG${ESC}[0m`)).toBe('DEBUG')
  })

  /**
   * Anything that normalises a line on the way here may drop the escape and
   * leave the tail, which is the half that is actually visible.
   */
  it('takes them out even when the escape byte is already gone', () => {
    expect(stripAnsi('[34mDEBUG[0m')).toBe('DEBUG')
  })

  it('leaves a line that was never coloured alone', () => {
    expect(stripAnsi('started listening on 0.0.0.0:8080')).toBe(
      'started listening on 0.0.0.0:8080',
    )
  })
})

describe('toRecord', () => {
  it('reads a tracing line into its parts', () => {
    const record = toRecord(line(TRACING))

    expect(record.level).toBe('debug')
    expect(record.target).toBe('rustls::client::hs')
    expect(record.text).toBe('Using ciphersuite TLS13_AES_256_GCM_SHA384')
  })

  /**
   * The platform already stamps every line with the moment its container
   * wrote it. Two clocks in one row is a column of noise beside a column of
   * the same thing.
   */
  it('drops the timestamp the product wrote itself', () => {
    expect(toRecord(line(TRACING)).text).not.toContain('2026-09-16')
  })

  it('reads the shape a JVM writes', () => {
    const record = toRecord(
      line('2026-09-16 07:34:03,166 INFO  [org.keycloak.services] Realm imported', 'keycloak'),
    )

    expect(record.level).toBe('info')
    expect(record.target).toBe('org.keycloak.services')
    expect(record.text).toBe('Realm imported')
  })

  it('reads the shape nginx writes, whose severity is bracketed and lower case', () => {
    const record = toRecord(line('2026/04/22 09:39:36 [notice] 1#1: start worker process 14'))

    expect(record.level).toBe('info')
    expect(record.text).toBe('1#1: start worker process 14')
  })

  /**
   * Reading a bare lower-case `info` as a severity would find one in half the
   * sentences a product logs. Brackets, at the front, or nothing.
   */
  it('does not read a bare lower-case severity word as the level', () => {
    expect(toRecord(line('rotating the info cache')).level).toBeNull()
  })

  /**
   * Half a pretty-printed struct, which `tracing` emits one line at a time.
   * It has no level and names no module, and inventing either would file a
   * fragment as a record of its own.
   */
  it('leaves a continuation line as text, with nothing claimed about it', () => {
    const record = toRecord(line('  cipher_suite: TLS13_AES_256_GCM_SHA384,'))

    expect(record.level).toBeNull()
    expect(record.target).toBeNull()
    // Indentation kept: on half a pretty-printed struct it is the only thing
    // saying which struct the fragment belongs to.
    expect(record.text).toBe('  cipher_suite: TLS13_AES_256_GCM_SHA384,')
  })

  /**
   * "the deployment failed" is a sentence, not a severity. A search over the
   * whole message finds a level in every other paragraph.
   */
  it('does not take a severity word deep in a sentence for the level', () => {
    const record = toRecord(
      line('could not reach the identity provider, so the health probe will error next tick'),
    )

    expect(record.level).toBeNull()
  })

  it('keeps a message that names no module whole', () => {
    expect(toRecord(line('INFO listening on 0.0.0.0:8080')).text).toBe('listening on 0.0.0.0:8080')
  })
})

describe('clears', () => {
  const at = (level: LogRecord['level']): LogRecord => ({
    at: '2026-09-16T07:34:03Z',
    source: 'ferriskey-api',
    level,
    target: null,
    text: 'something happened',
  })

  it('keeps what is at or above the floor', () => {
    expect(clears(at('error'), 'warn')).toBe(true)
    expect(clears(at('warn'), 'warn')).toBe(true)
  })

  it('drops what is below it', () => {
    expect(clears(at('debug'), 'warn')).toBe(false)
    expect(clears(at('trace'), 'info')).toBe(false)
  })

  /**
   * The one thing a log view must never do is delete evidence. A line we
   * failed to read is not a line we are entitled to hide.
   */
  it('keeps a line whose level could not be read, whatever the floor', () => {
    expect(clears(at(null), 'error')).toBe(true)
  })
})

describe('narrow', () => {
  const records = [
    toRecord(line(TRACING)),
    toRecord(line('2026-09-16T07:34:08.155928Z  WARN tower_http::trace: probe was slow')),
    toRecord(line('2026-09-16T07:34:09.155928Z ERROR aether::db: connection refused', 'postgres')),
  ]

  it('changes nothing at the bottom of the scale with nothing searched for', () => {
    expect(narrow(records, 'trace', '  ')).toHaveLength(3)
  })

  it('raises the floor', () => {
    expect(narrow(records, 'warn', '').map((found) => found.level)).toEqual(['warn', 'error'])
  })

  it('searches the module as well as the message', () => {
    expect(narrow(records, 'trace', 'tower_http')).toHaveLength(1)
  })

  it('searches the container', () => {
    expect(narrow(records, 'trace', 'postgres')).toHaveLength(1)
  })

  it('applies both at once', () => {
    expect(narrow(records, 'error', 'connection')).toHaveLength(1)
    expect(narrow(records, 'error', 'ciphersuite')).toHaveLength(0)
  })
})
