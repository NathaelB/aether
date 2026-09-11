import type { Schemas } from '@/api/api.client'

export const AUTO_UPGRADE_LABELS: Record<Schemas.AutoUpgradePolicy, string> = {
  manual: 'Nothing automatic',
  patch: 'Patches only',
  patch_and_minor: 'Patches and minor versions',
}

export const AUTO_UPGRADE_DESCRIPTIONS: Record<Schemas.AutoUpgradePolicy, string> = {
  manual: 'Every upgrade waits for someone to ask for it.',
  patch: 'Fixes are applied during your maintenance window. Nothing else is.',
  patch_and_minor: 'Fixes and new features are applied during your maintenance window.',
}

/**
 * A major version is never applied automatically, whatever the policy says.
 * The API cannot express it either, so this is a statement of what already
 * holds rather than a second place it is decided.
 */
export const MAJOR_ALWAYS_ASKS = 'A new major version always waits for you to approve it.'

export const WEEKDAYS = [
  { value: 'mon', label: 'Monday' },
  { value: 'tue', label: 'Tuesday' },
  { value: 'wed', label: 'Wednesday' },
  { value: 'thu', label: 'Thursday' },
  { value: 'fri', label: 'Friday' },
  { value: 'sat', label: 'Saturday' },
  { value: 'sun', label: 'Sunday' },
] as const

/**
 * The zones offered by default. A free text field would accept anything and
 * fail at the API; these are the ones a customer is most likely to mean, with
 * the browser's own zone first so the common case takes no thought.
 */
export function offeredTimezones(): string[] {
  const common = [
    'UTC',
    'Europe/Paris',
    'Europe/London',
    'Europe/Berlin',
    'America/New_York',
    'America/Los_Angeles',
    'Asia/Tokyo',
    'Australia/Sydney',
  ]

  let local = 'UTC'
  try {
    local = Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'
  } catch {
    // A browser that will not tell us falls back to UTC, which is always valid.
  }

  return [local, ...common.filter((zone) => zone !== local)]
}

/**
 * Without a window nothing is applied automatically, whatever the policy says.
 * The screen has to say so, because a customer who picked "patches only" and
 * left the window empty has chosen something that does nothing.
 */
export function policyIsInert(
  policy: Schemas.AutoUpgradePolicy,
  hasWindow: boolean,
): boolean {
  return policy !== 'manual' && !hasWindow
}

export function describeWindow(window: Schemas.MaintenanceWindow | null): string {
  if (!window) return 'No window, so nothing is applied automatically.'

  const day = WEEKDAYS.find((candidate) => candidate.value === window.day)?.label ?? window.day
  const hours = Math.floor(window.duration / 60)
  const minutes = window.duration % 60
  const length = hours > 0 ? `${hours}h${minutes > 0 ? ` ${minutes}m` : ''}` : `${minutes}m`

  return `${day} at ${window.start} for ${length}, ${window.timezone}`
}
