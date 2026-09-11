import { describe, expect, it } from 'vitest'
import { KEPT_LINES, keepRecent, readFrames, toLogLine, type LogLine } from './stream'

function line(message: string): LogLine {
  return { at: '2026-09-11T00:00:00Z', source: 'keycloak', message }
}

describe('readFrames', () => {
  it('reads a whole event', () => {
    const { frames, rest } = readFrames('event: line\ndata: {"message":"hello"}\n\n')

    expect(frames).toEqual([{ event: 'line', data: '{"message":"hello"}' }])
    expect(rest).toBe('')
  })

  /**
   * A chunk boundary lands mid line often enough that parsing each chunk on
   * its own drops roughly one event per read.
   */
  it('holds on to a half arrived event until the rest turns up', () => {
    const first = readFrames('event: line\ndata: {"mess')

    expect(first.frames).toEqual([])

    const second = readFrames(first.rest + 'age":"hello"}\n\n')

    expect(second.frames).toHaveLength(1)
    expect(second.frames[0].data).toBe('{"message":"hello"}')
  })

  it('reads several events out of one chunk', () => {
    const { frames } = readFrames(
      'event: line\ndata: {"message":"a"}\n\nevent: line\ndata: {"message":"b"}\n\n',
    )

    expect(frames).toHaveLength(2)
  })

  /** The keep-alive comment the server sends to hold the connection open. */
  it('ignores a frame carrying no data', () => {
    const { frames } = readFrames(':keep-alive\n\n')

    expect(frames).toEqual([])
  })
})

describe('toLogLine', () => {
  it('reads a line', () => {
    const parsed = toLogLine({
      event: 'line',
      data: '{"at":"2026-09-11T00:00:00Z","source":"keycloak","message":"started"}',
    })

    expect(parsed?.message).toBe('started')
    expect(parsed?.source).toBe('keycloak')
  })

  it('ignores an event that is not a line', () => {
    expect(toLogLine({ event: 'ping', data: '{}' })).toBeNull()
  })

  /**
   * One unreadable frame is one line lost, not a reason to tear the stream
   * down and lose every line after it.
   */
  it('drops a frame it cannot read rather than failing', () => {
    expect(toLogLine({ event: 'line', data: 'not json' })).toBeNull()
    expect(toLogLine({ event: 'line', data: '{"source":"x"}' })).toBeNull()
  })
})

describe('keepRecent', () => {
  it('keeps everything while there is room', () => {
    expect(keepRecent([line('a')], [line('b')]).map((l) => l.message)).toEqual(['a', 'b'])
  })

  /**
   * A busy instance produces more in a minute than anyone reads. Holding all
   * of them turns a log view into a memory leak with a scrollbar.
   */
  it('drops the oldest once the ceiling is reached', () => {
    const held = Array.from({ length: KEPT_LINES }, (_, index) => line(`old ${index}`))

    const kept = keepRecent(held, [line('newest')])

    expect(kept).toHaveLength(KEPT_LINES)
    expect(kept[kept.length - 1].message).toBe('newest')
    expect(kept[0].message).toBe('old 1')
  })
})
