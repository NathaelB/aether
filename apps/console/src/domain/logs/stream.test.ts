import { describe, expect, it } from 'vitest'
import {
  KEPT_LINES,
  backoffMs,
  dropOverlap,
  keepRecent,
  readFrames,
  toLogLine,
  toSessionEnd,
  whyNotReopened,
  type LogLine,
} from './stream'

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

describe('toSessionEnd', () => {
  it('reads why a session is over', () => {
    expect(toSessionEnd({ event: 'ended', data: '{"reason":"unreadable"}' })).toBe('unreadable')
  })

  it('is nothing at all for a frame that is not an ending', () => {
    expect(toSessionEnd({ event: 'line', data: '{"message":"hello"}' })).toBeNull()
  })

  /**
   * An end nobody here recognises is still an end. Reading it as no end at
   * all would leave the screen waiting on a session that is over.
   */
  it('still counts a reason it does not recognise as an ending', () => {
    expect(toSessionEnd({ event: 'ended', data: '{"reason":"something new"}' })).toBe('finished')
    expect(toSessionEnd({ event: 'ended', data: 'not json' })).toBe('finished')
  })
})

describe('dropOverlap', () => {
  const at = (stamp: string, message: string): LogLine => ({
    at: stamp,
    source: 'keycloak',
    message,
  })

  it('keeps everything when nothing is held', () => {
    const arriving = [at('2026-09-11T00:00:01Z', 'a')]

    expect(dropOverlap([], arriving)).toEqual(arriving)
  })

  /** Reopening asks for the same window, so the first lines back are old. */
  it('drops the lines the previous session already showed', () => {
    const held = [at('2026-09-11T00:00:01Z', 'a'), at('2026-09-11T00:00:02Z', 'b')]
    const arriving = [...held, at('2026-09-11T00:00:03Z', 'c')]

    expect(dropOverlap(held, arriving).map((line) => line.message)).toEqual(['c'])
  })

  /**
   * Past the last line held, two identical lines are two things that
   * happened. Dropping the second would be the log view lying the other way.
   */
  it('keeps a repeat that happened after the last line held', () => {
    const held = [at('2026-09-11T00:00:01Z', 'restarting')]
    const arriving = [at('2026-09-11T00:00:01Z', 'restarting'), at('2026-09-11T00:00:09Z', 'restarting')]

    expect(dropOverlap(held, arriving)).toHaveLength(1)
  })

  /**
   * The stamps carry fractional seconds only when they have any, so text
   * order and time order disagree exactly where the frontier sits.
   */
  it('compares stamps as instants, not as text', () => {
    const held = [at('2026-09-11T00:00:01Z', 'a')]
    const arriving = [at('2026-09-11T00:00:01.500Z', 'b')]

    expect(dropOverlap(held, arriving).map((line) => line.message)).toEqual(['b'])
  })

  it('keeps a line from the overlap that is genuinely different', () => {
    const held = [at('2026-09-11T00:00:01Z', 'a')]
    const arriving = [at('2026-09-11T00:00:01Z', 'a'), at('2026-09-11T00:00:01Z', 'b')]

    expect(dropOverlap(held, arriving).map((line) => line.message)).toEqual(['b'])
  })
})

describe('backoffMs', () => {
  /**
   * A session reaching its ceiling is the ordinary case. Making somebody wait
   * a second for it would be a pause they did not ask for.
   */
  it('reopens immediately after a session that was running', () => {
    expect(backoffMs(1)).toBe(0)
  })

  it('waits longer each time reopening keeps failing', () => {
    expect(backoffMs(2)).toBe(1000)
    expect(backoffMs(3)).toBe(2000)
    expect(backoffMs(4)).toBe(4000)
  })

  it('stops growing, so a tab left open keeps trying without spinning', () => {
    expect(backoffMs(20)).toBe(15_000)
  })
})

describe('whyNotReopened', () => {
  it('reopens a session that simply finished', () => {
    expect(whyNotReopened({ kind: 'ended', reason: 'finished' })).toBeNull()
  })

  it('reopens one the control plane gave up on, and one that just dropped', () => {
    expect(whyNotReopened({ kind: 'ended', reason: 'silent' })).toBeNull()
    expect(whyNotReopened({ kind: 'dropped' })).toBeNull()
  })

  /** Another session would fail the same way, so say so instead. */
  it('does not reopen a deployment whose pods cannot be read', () => {
    expect(whyNotReopened({ kind: 'ended', reason: 'unreadable' })).toContain('no pods')
  })
})
