import { describe, expect, it } from 'vitest'
import type { Schemas } from '@/api/api.client'
import { actionLabel, actionMarkers, maxCount, toBars, xFraction } from './histogram'

function action(actionType: string, createdAt: string): Schemas.Action {
  return {
    id: 'action-1',
    dataplane_id: 'dataplane-1',
    deployment_id: 'deployment-1',
    action_type: actionType,
    target: { id: 'deployment-1', kind: 'Deployment' },
    payload: { data: null },
    version: 1,
    status: 'Pending',
    metadata: { created_at: createdAt, constraints: {}, source: 'System' },
  }
}

describe('toBars', () => {
  it('reads a bucket into epoch milliseconds and its count', () => {
    const bars = toBars([{ start: '2026-09-20T12:00:00Z', count: 3 }])

    expect(bars).toEqual([{ at: Date.parse('2026-09-20T12:00:00Z'), count: 3 }])
  })

  it('sorts oldest first regardless of the order the index answered in', () => {
    const bars = toBars([
      { start: '2026-09-20T12:01:00Z', count: 1 },
      { start: '2026-09-20T12:00:00Z', count: 2 },
    ])

    expect(bars.map((bar) => bar.count)).toEqual([2, 1])
  })

  it('drops a bucket whose stamp cannot be read', () => {
    expect(toBars([{ start: 'not a time', count: 5 }])).toEqual([])
  })
})

describe('maxCount', () => {
  it('is the tallest bar', () => {
    expect(maxCount([{ at: 0, count: 2 }, { at: 1, count: 9 }, { at: 2, count: 4 }])).toBe(9)
  })

  it('is never zero, so an empty window still has a scale', () => {
    expect(maxCount([])).toBe(1)
    expect(maxCount([{ at: 0, count: 0 }])).toBe(1)
  })
})

describe('actionLabel', () => {
  it('reads the last segment of a namespaced action type', () => {
    expect(actionLabel('deployment.upgrade')).toBe('upgrade')
    expect(actionLabel('deployment.restore')).toBe('restore')
    expect(actionLabel('deployment.drill')).toBe('drill')
  })

  it('spaces out an underscored segment', () => {
    expect(actionLabel('deployment.network_access')).toBe('network access')
  })

  it('falls back to the whole string when there is no separator', () => {
    expect(actionLabel('upgrade')).toBe('upgrade')
  })
})

describe('actionMarkers', () => {
  const from = Date.parse('2026-09-20T12:00:00Z')
  const to = Date.parse('2026-09-20T13:00:00Z')

  it('keeps an action that landed inside the window', () => {
    const markers = actionMarkers([action('deployment.upgrade', '2026-09-20T12:30:00Z')], from, to)

    expect(markers).toEqual([{ at: Date.parse('2026-09-20T12:30:00Z'), label: 'upgrade' }])
  })

  it('drops an action before the window and one at or after its end', () => {
    const markers = actionMarkers(
      [
        action('deployment.upgrade', '2026-09-20T11:59:59Z'),
        action('deployment.restore', '2026-09-20T13:00:00Z'),
      ],
      from,
      to,
    )

    expect(markers).toEqual([])
  })

  it('keeps an action landing exactly on the window start, inclusive', () => {
    const markers = actionMarkers([action('deployment.drill', '2026-09-20T12:00:00Z')], from, to)

    expect(markers).toHaveLength(1)
  })

  it('orders markers oldest first', () => {
    const markers = actionMarkers(
      [
        action('deployment.restore', '2026-09-20T12:40:00Z'),
        action('deployment.upgrade', '2026-09-20T12:10:00Z'),
      ],
      from,
      to,
    )

    expect(markers.map((marker) => marker.label)).toEqual(['upgrade', 'restore'])
  })
})

describe('xFraction', () => {
  const from = 1000
  const to = 2000

  it('places the start of the window at 0 and the end at 1', () => {
    expect(xFraction(from, from, to)).toBe(0)
    expect(xFraction(to, from, to)).toBe(1)
  })

  it('places the midpoint halfway across', () => {
    expect(xFraction(1500, from, to)).toBe(0.5)
  })

  it('clamps an instant outside the window rather than drawing off the axis', () => {
    expect(xFraction(0, from, to)).toBe(0)
    expect(xFraction(3000, from, to)).toBe(1)
  })

  it('does not divide by zero when the window is empty', () => {
    expect(xFraction(500, 1000, 1000)).toBe(0)
  })
})
