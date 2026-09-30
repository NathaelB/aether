import { describe, expect, it } from 'vitest'
import type { Schemas } from '@/api/api.client'
import { ApiRequestError } from '@/api/api.fetch'
import { availabilityView, formatAvailability } from './availability'

const win = (p: number | null, full = true): Schemas.UptimeWindow => ({
  uptime_percent: p,
  covers_full_window: full,
})

const response = (
  d: Schemas.UptimeWindow,
  w: Schemas.UptimeWindow,
  m: Schemas.UptimeWindow
): Schemas.DeploymentUptimeResponse => ({
  data: { deployment_id: 'd', uptime_24h: d, uptime_7d: w, uptime_30d: m },
})

describe('formatAvailability', () => {
  it('uses three decimals', () => {
    expect(formatAvailability(99.992)).toBe('99.992%')
    expect(formatAvailability(99.99074)).toBe('99.991%')
    expect(formatAvailability(0)).toBe('0.000%')
    expect(formatAvailability(100)).toBe('100.000%')
  })

  it('never rounds a value below 100 up to 100', () => {
    expect(formatAvailability(99.9996)).toBe('99.999%')
    expect(formatAvailability(99.99999)).toBe('99.999%')
  })

  it('shows unknown, never 100, when nothing was observed', () => {
    expect(formatAvailability(null)).toBe('Unknown')
    expect(formatAvailability(undefined)).toBe('Unknown')
  })
})

describe('availabilityView', () => {
  it('is hidden without the right', () => {
    const view = availabilityView(false, {
      data: response(win(1), win(1), win(1)),
      error: null,
      isPending: false,
    })
    expect(view.kind).toBe('hidden')
  })

  it('is loading while pending', () => {
    expect(availabilityView(true, { error: null, isPending: true }).kind).toBe('loading')
  })

  it('reads a 404 as no data yet', () => {
    const error = new ApiRequestError(404, 'none')
    expect(availabilityView(true, { error, isPending: false }).kind).toBe('no-data')
  })

  it('reads other failures as an error', () => {
    const error = new ApiRequestError(500, 'boom')
    expect(availabilityView(true, { error, isPending: false }).kind).toBe('error')
  })

  it('headlines 30 days and flags partial and unknown windows', () => {
    const view = availabilityView(true, {
      data: response(win(100), win(99.5, false), win(99.992)),
      error: null,
      isPending: false,
    })
    expect(view).toEqual({
      kind: 'ready',
      headline: { label: 'Last 30 days', value: '99.992%', partial: false },
      others: [
        { label: 'Last 24 hours', value: '100.000%', partial: false },
        { label: 'Last 7 days', value: '99.500%', partial: true },
      ],
    })
  })

  it('shows null as unknown without a partial flag', () => {
    const view = availabilityView(true, {
      data: response(win(null, false), win(null), win(null, false)),
      error: null,
      isPending: false,
    })
    if (view.kind !== 'ready') throw new Error('expected ready')
    expect(view.headline).toEqual({ label: 'Last 30 days', value: 'Unknown', partial: false })
  })
})
