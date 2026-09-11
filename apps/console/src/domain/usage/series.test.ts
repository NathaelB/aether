import { describe, expect, it } from 'vitest'
import {
  aggregationFor,
  axisTicks,
  formatCount,
  growth,
  segments,
  toSeries,
  type Bucket,
  type Point,
} from './series'

const FROM = Date.parse('2026-09-11T00:00:00Z')
const UNTIL = Date.parse('2026-09-11T01:00:00Z')

function bucket(minute: number, value: number): Bucket {
  return { bucket: new Date(FROM + minute * 60_000).toISOString(), value }
}

describe('aggregationFor', () => {
  /**
   * The same person active in three minutes is one user, not three. The read
   * side already refuses to sum it, and a chart that summed anyway would
   * disagree with the number printed beside it.
   */
  it('never adds up active users', () => {
    expect(aggregationFor('active_users')).toBe('last')
    expect(aggregationFor('requests')).toBe('sum')
    expect(aggregationFor('logins')).toBe('sum')
    expect(aggregationFor('token_events')).toBe('sum')
  })
})

describe('toSeries', () => {
  it('adds up the minutes that fall in one slot', () => {
    const series = toSeries([bucket(0, 3), bucket(1, 4), bucket(30, 10)], FROM, UNTIL, 2, 'sum')

    expect(series.map((point) => point.value)).toEqual([7, 10])
  })

  it('takes the last reading for a level rather than adding', () => {
    const series = toSeries([bucket(0, 3), bucket(1, 9)], FROM, UNTIL, 2, 'last')

    expect(series[0].value).toBe(9)
  })

  /**
   * The reason this whole file exists. A slot nobody reported is a gap, and a
   * zero would say the deployment was reached and had no traffic.
   */
  it('leaves a slot with nothing reported empty rather than zero', () => {
    const series = toSeries([bucket(0, 5)], FROM, UNTIL, 2, 'sum')

    expect(series[0].value).toBe(5)
    expect(series[1].value).toBeNull()
  })

  it('ignores a bucket outside the period', () => {
    const before = { bucket: new Date(FROM - 60_000).toISOString(), value: 99 }
    const after = { bucket: new Date(UNTIL + 60_000).toISOString(), value: 99 }

    const series = toSeries([before, after, bucket(0, 1)], FROM, UNTIL, 2, 'sum')

    expect(series.map((point) => point.value)).toEqual([1, null])
  })

  it('ignores a bucket whose instant cannot be read', () => {
    const series = toSeries([{ bucket: 'not a date', value: 5 }], FROM, UNTIL, 2, 'sum')

    expect(series.every((point) => point.value === null)).toBe(true)
  })
})

describe('segments', () => {
  function points(...values: (number | null)[]): Point[] {
    return values.map((value, index) => ({ at: FROM + index * 60_000, value }))
  }

  /**
   * One path across a gap draws a straight line through hours nobody
   * reported, which is the chart telling a story the data does not support.
   */
  it('breaks the line where nothing was reported', () => {
    const runs = segments(points(1, 2, null, 4, 5))

    expect(runs.length).toBe(2)
    expect(runs[0].map((point) => point.value)).toEqual([1, 2])
    expect(runs[1].map((point) => point.value)).toEqual([4, 5])
  })

  it('gives nothing back when nothing was reported at all', () => {
    expect(segments(points(null, null))).toEqual([])
  })

  it('keeps a single run whole', () => {
    expect(segments(points(1, 2, 3))).toHaveLength(1)
  })
})

describe('axisTicks', () => {
  function points(...values: (number | null)[]): Point[] {
    return values.map((value, index) => ({ at: index, value }))
  }

  /**
   * A label at 100 above a line that peaks at 37 tells the reader something
   * false about the shape they are looking at.
   */
  it('only names values the data reaches', () => {
    const ticks = axisTicks(points(4, 37, 12))

    expect(Math.min(...ticks)).toBe(4)
    expect(Math.max(...ticks)).toBe(37)
  })

  it('names one value when the line is flat', () => {
    expect(axisTicks(points(8, 8, 8))).toEqual([8])
  })

  it('names nothing when nothing was reported', () => {
    expect(axisTicks(points(null, null))).toEqual([])
  })
})

describe('growth', () => {
  function points(...values: (number | null)[]): Point[] {
    return values.map((value, index) => ({ at: index, value }))
  }

  it('compares the second half of the period with the first', () => {
    expect(growth(points(10, 10, 15, 15), 'sum')).toBeCloseTo(0.5)
  })

  /**
   * Zero would read as flat. A period with nothing to compare against has no
   * growth, which is a different statement.
   */
  it('says nothing rather than zero when a half reported nothing', () => {
    expect(growth(points(null, null, 5, 5), 'sum')).toBeNull()
    expect(growth(points(5, 5, null, null), 'sum')).toBeNull()
  })

  it('says nothing when the first half was zero', () => {
    expect(growth(points(0, 0, 5, 5), 'sum')).toBeNull()
  })
})

describe('formatCount', () => {
  it('keeps small numbers exact and shortens large ones', () => {
    expect(formatCount(42)).toBe('42')
    expect(formatCount(1500)).toBe('1.5k')
    expect(formatCount(2_400_000)).toBe('2.4M')
  })
})
