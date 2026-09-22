import { describe, expect, it } from 'vitest'
import {
  DEFAULT_SEARCH_LEVEL,
  DEFAULT_SEARCH_WINDOW_MINUTES,
  SEARCH_LEVELS,
  SEARCH_WINDOWS,
  asResultLevel,
  buildSearchRequest,
  describeResults,
  facetShares,
  formatCount,
  formatTimestamp,
  levelBreakdown,
  orderFacet,
  resolveWindow,
  shortId,
  summarize,
  whySearchFailed,
} from './search'

describe('resolveWindow', () => {
  it('anchors the span to the instant it is given, not to when it runs', () => {
    const now = new Date('2026-09-20T12:00:00.000Z')

    expect(resolveWindow(60, now)).toEqual({
      from: '2026-09-20T11:00:00.000Z',
      to: '2026-09-20T12:00:00.000Z',
    })
  })

  it('scales with the preset', () => {
    const now = new Date('2026-09-20T12:00:00.000Z')

    expect(resolveWindow(60 * 24, now).from).toBe('2026-09-19T12:00:00.000Z')
  })
})

describe('buildSearchRequest', () => {
  const now = new Date('2026-09-20T12:00:00.000Z')

  it('carries the deployment, the resolved window and the floor', () => {
    const request = buildSearchRequest(
      { windowMinutes: 60, floor: 'warn', text: '' },
      'deployment-1',
      now,
    )

    expect(request).toEqual({
      from: '2026-09-20T11:00:00.000Z',
      to: '2026-09-20T12:00:00.000Z',
      level_floor: 'warn',
      deployment_id: 'deployment-1',
    })
  })

  it('trims the free text and leaves it out once empty', () => {
    expect(
      buildSearchRequest({ windowMinutes: 60, floor: 'info', text: '   ' }, 'd', now).q,
    ).toBeUndefined()

    expect(
      buildSearchRequest({ windowMinutes: 60, floor: 'info', text: '  token expired  ' }, 'd', now)
        .q,
    ).toBe('token expired')
  })
})

describe('SEARCH_LEVELS', () => {
  it('is ordered least severe first, matching the control plane', () => {
    expect(SEARCH_LEVELS).toEqual(['trace', 'debug', 'info', 'warn', 'error', 'fatal'])
  })

  it('does not offer unknown as a floor', () => {
    expect(SEARCH_LEVELS).not.toContain('unknown')
  })
})

describe('defaults', () => {
  it('lands a customer on the hour the acceptance criterion asks about', () => {
    expect(DEFAULT_SEARCH_WINDOW_MINUTES).toBe(60)
    expect(SEARCH_WINDOWS.map((window) => window.minutes)).toContain(DEFAULT_SEARCH_WINDOW_MINUTES)
  })

  it('defaults to a floor that is neither silent nor noisy', () => {
    expect(DEFAULT_SEARCH_LEVEL).toBe('info')
  })

  it('offers a preset matching the index’s own retention', () => {
    expect(SEARCH_WINDOWS.map((window) => window.minutes)).toContain(60 * 24 * 30)
  })
})

describe('asResultLevel', () => {
  it('reads a level the screen knows about as itself', () => {
    for (const level of SEARCH_LEVELS) {
      expect(asResultLevel(level)).toBe(level)
    }
  })

  it('reads the index’s own unknown as unknown', () => {
    expect(asResultLevel('unknown')).toBe('unknown')
  })

  /** A value nobody promised is read as unknown rather than dropped or guessed at. */
  it('reads anything else as unknown too', () => {
    expect(asResultLevel('critical')).toBe('unknown')
    expect(asResultLevel('')).toBe('unknown')
  })
})

describe('formatTimestamp', () => {
  it('keeps the date, unlike the live tail’s time-only stamp', () => {
    expect(formatTimestamp('2026-09-11T14:05:09Z')).toBe('2026-09-11 14:05:09Z')
  })

  it('drops sub-second precision', () => {
    expect(formatTimestamp('2026-09-11T14:05:09.842Z')).toBe('2026-09-11 14:05:09Z')
  })

  it('leaves a stamp it cannot read alone', () => {
    expect(formatTimestamp('not a time')).toBe('not a time')
  })
})

describe('summarize', () => {
  it('says plainly when nothing matched', () => {
    expect(summarize(0, 0)).toBe('No matching lines')
  })

  it('counts a single match in the singular', () => {
    expect(summarize(1, 1)).toBe('1 line')
  })

  it('states the total once every match is shown', () => {
    expect(summarize(42, 42)).toBe('42 lines')
  })

  it('tells a capped result apart from a complete one', () => {
    expect(summarize(812, 200)).toBe('200 of 812 lines')
  })
})

describe('whySearchFailed', () => {
  it('names the installation gap plainly, distinct from an empty result', () => {
    expect(whySearchFailed(409)).toMatch(/not set up/)
  })

  it('tells a refusal apart from a missing deployment', () => {
    expect(whySearchFailed(403)).not.toBe(whySearchFailed(404))
  })

  it('falls back to the status code for anything unnamed', () => {
    expect(whySearchFailed(500)).toContain('500')
  })
})

describe('orderFacet', () => {
  it('sorts the busiest value first', () => {
    expect(
      orderFacet([
        { value: 'a', count: 1 },
        { value: 'b', count: 9 },
      ]),
    ).toEqual([
      { value: 'b', count: 9 },
      { value: 'a', count: 1 },
    ])
  })

  it('breaks a tie alphabetically rather than leaving it to arrival order', () => {
    expect(
      orderFacet([
        { value: 'zebra', count: 3 },
        { value: 'ant', count: 3 },
      ]),
    ).toEqual([
      { value: 'ant', count: 3 },
      { value: 'zebra', count: 3 },
    ])
  })

  it('does not mutate the buckets it was handed', () => {
    const buckets = [
      { value: 'a', count: 1 },
      { value: 'b', count: 9 },
    ]
    orderFacet(buckets)
    expect(buckets[0].value).toBe('a')
  })
})

describe('facetShares', () => {
  it('orders the same way orderFacet does', () => {
    expect(
      facetShares([
        { value: 'a', count: 1 },
        { value: 'b', count: 9 },
      ]).map((share) => share.value),
    ).toEqual(['b', 'a'])
  })

  it('shares against the facet’s own total, not total_hits', () => {
    const shares = facetShares([
      { value: 'frontend', count: 3 },
      { value: 'worker', count: 1 },
    ])

    expect(shares.find((share) => share.value === 'frontend')?.percent).toBe(75)
    expect(shares.find((share) => share.value === 'worker')?.percent).toBe(25)
  })

  it('is empty, not divided by zero, once the facet has no buckets', () => {
    expect(facetShares([])).toEqual([])
  })
})

describe('levelBreakdown', () => {
  it('orders least severe first, with unknown last', () => {
    const segments = levelBreakdown([
      { value: 'unknown', count: 10 },
      { value: 'error', count: 5 },
      { value: 'info', count: 20 },
    ])

    expect(segments.map((segment) => segment.level)).toEqual(['info', 'error', 'unknown'])
  })

  it('leaves out a level nothing matched', () => {
    const segments = levelBreakdown([{ value: 'warn', count: 6 }])
    expect(segments).toHaveLength(1)
  })

  it('shares the bar by count', () => {
    const segments = levelBreakdown([
      { value: 'info', count: 3 },
      { value: 'error', count: 1 },
    ])

    expect(segments.find((segment) => segment.level === 'info')?.percent).toBe(75)
    expect(segments.find((segment) => segment.level === 'error')?.percent).toBe(25)
  })

  it('is empty, not divided by zero, once nothing matched at all', () => {
    expect(levelBreakdown([])).toEqual([])
  })
})

describe('shortId', () => {
  it('leaves a short value alone', () => {
    expect(shortId('abc123')).toBe('abc123')
  })

  it('shortens a uuid to a run a reader can scan', () => {
    expect(shortId('9f8e7d6c-1234-5678-9abc-def012345678')).toBe('9f8e7d6c…')
  })
})

describe('formatCount', () => {
  it('groups by thousands', () => {
    expect(formatCount(6840)).toBe('6,840')
  })

  it('leaves a small count alone', () => {
    expect(formatCount(42)).toBe('42')
  })
})

describe('describeResults', () => {
  it('says plainly when nothing matched, with the timing', () => {
    expect(describeResults(0, 0, 12)).toBe('No logs found in 12 ms')
  })

  it('states the total once every match is shown', () => {
    expect(describeResults(42, 42, 8)).toBe('42 logs found in 8 ms')
  })

  it('counts a single match in the singular', () => {
    expect(describeResults(1, 1, 5)).toBe('1 log found in 5 ms')
  })

  it('tells a capped result apart from a complete one, the reference’s lie this avoids', () => {
    expect(describeResults(6840, 200, 72)).toBe(
      'Showing the first 200 of 6,840 logs in 72 ms — narrow the search to see the rest',
    )
  })

  it('rounds a fractional client-measured duration', () => {
    expect(describeResults(1, 1, 5.6)).toBe('1 log found in 6 ms')
  })
})
