import type { Schemas } from '@/api/api.client'
import { compareVersions } from '@/domain/upgrades/version'

/**
 * The versions a new deployment may start on, newest first.
 *
 * Only what is available. A deprecated version can still be passed through on
 * the way somewhere else, but starting a new instance on one buys something
 * already on its way out; planned and withdrawn are not installable at all.
 */
export function installableVersions(releases: Schemas.Release[]): Schemas.Release[] {
  return releases
    .filter((release) => release.status === 'available')
    .sort((a, b) => compareVersions(b.id.version, a.id.version))
}

/** What the form should offer by default. */
export function newestInstallable(releases: Schemas.Release[]): string | null {
  return installableVersions(releases)[0]?.id.version ?? null
}

/**
 * Whether a release is offered to the whole estate.
 *
 * Derived from the rollout rather than stored beside it: there is one notion
 * of who receives what, and a second flag would be a second place to read
 * before believing the first.
 */
export function isGloballyAvailable(release: Schemas.Release): boolean {
  return release.rollout.percentage === 100 && !release.rollout.plans
}

/**
 * Who a release currently reaches, in words.
 *
 * The percentage alone reads as a setting; what an operator wants to know at
 * a glance is whether anybody is getting this version.
 */
export function audienceLabel(rollout: Schemas.Rollout): string {
  const pilots = rollout.pilot_organisations.length
  const pilotSuffix = pilots > 0 ? `, plus ${pilots} pilot${pilots > 1 ? 's' : ''}` : ''

  if (rollout.percentage === 100 && !rollout.plans) return 'Everyone'
  if (rollout.percentage === 0 && pilots === 0) return 'Nobody yet'

  const plans = rollout.plans ? ` on ${rollout.plans.join(', ')}` : ''
  return `${rollout.percentage}% of the estate${plans}${pilotSuffix}`
}

/** One `major.minor` line of a product, newest release first. */
export interface ReleaseLine {
  label: string
  releases: Schemas.Release[]
}

/**
 * The catalogue grouped by the line a release belongs to.
 *
 * Patches of one line belong together: an operator adding 26.7.4 is
 * continuing 26.7, not starting something new, and a flat list makes that
 * look like an unrelated entry every time.
 */
export function releaseLines(releases: Schemas.Release[]): ReleaseLine[] {
  const lines = new Map<string, Schemas.Release[]>()

  for (const release of releases) {
    const label = lineOf(release.id.version)
    if (!label) continue

    lines.set(label, [...(lines.get(label) ?? []), release])
  }

  return [...lines.entries()]
    .map(([label, held]) => ({
      label,
      releases: [...held].sort((a, b) => compareVersions(b.id.version, a.id.version)),
    }))
    .sort((a, b) => compareVersions(`${b.label}.0`, `${a.label}.0`))
}

/**
 * The patch an operator would add next to a line.
 *
 * Offered rather than imposed: it is right whenever patches are published in
 * order, which is almost always, and it is only a default in a field they can
 * still edit.
 */
export function nextPatchIn(line: ReleaseLine): string | null {
  const newest = line.releases[0]?.id.version
  if (!newest) return null

  const parts = newest.split('.')
  const patch = Number(parts[2])
  if (!Number.isInteger(patch)) return null

  return `${parts[0]}.${parts[1]}.${patch + 1}`
}

function lineOf(version: string): string | null {
  const [major, minor, patch] = version.split('.')
  if (!major || !minor || patch === undefined) return null

  return `${major}.${minor}`
}
