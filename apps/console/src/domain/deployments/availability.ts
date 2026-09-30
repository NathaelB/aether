import type { Schemas } from '@/api/api.client'
import { ApiRequestError } from '@/api/api.fetch'

export function formatAvailability(percent: number | null | undefined): string {
  if (percent === null || percent === undefined) return 'Unknown'
  const text = percent.toFixed(3)
  if (text === '100.000' && percent < 100) return '99.999%'
  return `${text}%`
}

export interface AvailabilityFigure {
  label: string
  value: string
  partial: boolean
}

export type AvailabilityView =
  | { kind: 'hidden' }
  | { kind: 'loading' }
  | { kind: 'no-data' }
  | { kind: 'error' }
  | { kind: 'ready'; headline: AvailabilityFigure; others: AvailabilityFigure[] }

function figure(label: string, window: Schemas.UptimeWindow): AvailabilityFigure {
  const value = formatAvailability(window.uptime_percent)
  return { label, value, partial: value !== 'Unknown' && !window.covers_full_window }
}

interface Query {
  data?: Schemas.DeploymentUptimeResponse
  error: unknown
  isPending: boolean
}

export function availabilityView(allowed: boolean, query: Query): AvailabilityView {
  if (!allowed) return { kind: 'hidden' }
  if (query.error instanceof ApiRequestError && query.error.status === 404) {
    return { kind: 'no-data' }
  }
  if (query.error) return { kind: 'error' }
  if (query.isPending || !query.data) return { kind: 'loading' }

  const { uptime_24h, uptime_7d, uptime_30d } = query.data.data
  return {
    kind: 'ready',
    headline: figure('Last 30 days', uptime_30d),
    others: [figure('Last 24 hours', uptime_24h), figure('Last 7 days', uptime_7d)],
  }
}
