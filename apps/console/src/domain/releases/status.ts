import type { Schemas } from '@/api/api.client'
import type { Tone } from '@/components/ui/status-badge'

export const RELEASE_STATUS_LABELS: Record<Schemas.ReleaseStatus, string> = {
  upcoming: 'Planned',
  available: 'Available',
  deprecated: 'Deprecated',
  withdrawn: 'Withdrawn',
}

export const RELEASE_STATUS_TONES: Record<Schemas.ReleaseStatus, Tone> = {
  upcoming: 'neutral',
  available: 'success',
  deprecated: 'warning',
  withdrawn: 'danger',
}

export const RISK_LABELS: Record<Schemas.BreakingRisk, string> = {
  none: 'Drop in',
  config: 'Config may need attention',
  breaking: 'Breaking',
}

export const RISK_TONES: Record<Schemas.BreakingRisk, Tone> = {
  none: 'neutral',
  config: 'warning',
  breaking: 'danger',
}

/**
 * A release only ever moves forward, so the screen offers only forward steps.
 * The API refuses the rest, but a button that exists to be rejected is a
 * button that should not be there.
 */
const ORDER: Schemas.ReleaseStatus[] = ['upcoming', 'available', 'deprecated', 'withdrawn']

export function nextStatuses(current: Schemas.ReleaseStatus): Schemas.ReleaseStatus[] {
  return ORDER.slice(ORDER.indexOf(current) + 1)
}

/**
 * Withdrawing a version that nothing runs is housekeeping. Withdrawing one
 * that is still serving traffic is an incident, and the screen should say so
 * before the click rather than after.
 */
export function withdrawalStrands(
  status: Schemas.ReleaseStatus,
  deployments: number,
): boolean {
  return status === 'withdrawn' && deployments > 0
}
