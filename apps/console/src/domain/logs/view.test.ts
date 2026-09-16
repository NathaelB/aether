import { describe, expect, it } from 'vitest'
import type { LogLine } from './stream'
import { SOURCE_TONES, asText, atTheEnd, narrow, readStamp, toneFor } from './view'

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

describe('narrow', () => {
  const lines = [line('started listening', 'api'), line('checkpoint complete', 'postgres')]

  it('keeps everything when nothing is being looked for', () => {
    expect(narrow(lines, '   ')).toHaveLength(2)
  })

  it('finds a word in the message, whatever the case', () => {
    expect(narrow(lines, 'CHECKPOINT').map((found) => found.source)).toEqual(['postgres'])
  })

  /** "the postgres one" narrows a screen as much as a word in the text does. */
  it('finds a container by name', () => {
    expect(narrow(lines, 'api').map((found) => found.message)).toEqual(['started listening'])
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
    expect(asText([line('started', 'api')], 'utc')).toBe('14:05:09 api started')
  })
})
