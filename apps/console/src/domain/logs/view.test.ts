import { describe, expect, it } from 'vitest'
import { toRecord } from './record'
import type { LogLine } from './stream'
import { SOURCE_TONES, asText, atTheEnd, readStamp, toneFor } from './view'

function line(message: string, source = 'ferriskey', at = '2026-09-11T14:05:09Z'): LogLine {
  return { at, source, message }
}

describe('toneFor', () => {
  it('gives one container the same tone every time', () => {
    expect(toneFor('ferriskey-api')).toBe(toneFor('ferriskey-api'))
  })

  it('stays inside the palette', () => {
    for (const source of ['a', 'ferriskey-api', 'postgres', '', 'a much longer container name']) {
      expect(toneFor(source)).toBeGreaterThanOrEqual(0)
      expect(toneFor(source)).toBeLessThan(SOURCE_TONES)
    }
  })

  it('tells two containers of one deployment apart', () => {
    expect(toneFor('ferriskey-api')).not.toBe(toneFor('postgres'))
  })
})

describe('readStamp', () => {
  it('reads the instance’s own time', () => {
    expect(readStamp('2026-09-11T14:05:09Z', 'utc')).toBe('14:05:09')
  })

  /** A stamp that cannot be read keeps its text rather than becoming a lie. */
  it('leaves a stamp it cannot read alone', () => {
    expect(readStamp('not a time', 'utc')).toBe('not a time')
  })
})

describe('atTheEnd', () => {
  it('is true at the bottom', () => {
    expect(atTheEnd({ scrollTop: 600, scrollHeight: 1000, clientHeight: 400 })).toBe(true)
  })

  /**
   * Following the tail leaves the scroller a few pixels short often enough
   * that an exact test would read as the reader having scrolled up.
   */
  it('is still true a few pixels short', () => {
    expect(atTheEnd({ scrollTop: 580, scrollHeight: 1000, clientHeight: 400 })).toBe(true)
  })

  it('is false once somebody has scrolled up to read', () => {
    expect(atTheEnd({ scrollTop: 100, scrollHeight: 1000, clientHeight: 400 })).toBe(false)
  })
})

describe('asText', () => {
  it('hands over one line per line, in the time on screen', () => {
    expect(asText([toRecord(line('started', 'api'))], 'utc')).toBe('14:05:09 api started')
  })

  /**
   * What is pasted into an issue is what was on the screen: read apart, and
   * without the colour codes that made it unreadable in the first place.
   */
  it('hands over the line as it was read, not as it arrived', () => {
    const copied = asText(
      [toRecord(line('\u001b[34mDEBUG\u001b[0m rustls::client::hs: handshake done', 'api'))],
      'utc',
    )

    expect(copied).toBe('14:05:09 api DEBUG rustls::client::hs handshake done')
  })
})
