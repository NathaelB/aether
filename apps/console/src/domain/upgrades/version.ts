import type { Schemas } from '@/api/api.client'

export type VersionChange = 'patch' | 'minor' | 'major'

type Parts = [number, number, number]

/**
 * Build metadata and prerelease tags are dropped rather than compared. The
 * catalogue is the authority on what may be installed, so the screen only has
 * to order what it is given.
 */
function parse(version: string): Parts | null {
  const core = version.trim().split(/[-+]/)[0]
  const parts = core.split('.')
  if (parts.length !== 3) return null

  const numbers = parts.map((part) => (/^\d+$/.test(part) ? Number(part) : Number.NaN))
  if (numbers.some(Number.isNaN)) return null

  return numbers as Parts
}

/**
 * Whether this reads as a version at all.
 *
 * The platform refuses anything that is not exact, `latest` included, because
 * a deployment records the version it runs. Saying so in the form is kinder
 * than letting the API say it after the submit.
 */
export function isVersion(value: string): boolean {
  return parse(value) !== null
}

export function compareVersions(a: string, b: string): number {
  const left = parse(a)
  const right = parse(b)
  if (!left || !right) return 0

  for (let i = 0; i < 3; i++) {
    if (left[i] !== right[i]) return left[i] - right[i]
  }
  return 0
}

/**
 * What moving from one version to the other means. Null when the target is
 * not ahead, which is not an upgrade and has nothing to classify.
 */
export function changeBetween(from: string, to: string): VersionChange | null {
  const current = parse(from)
  const target = parse(to)
  if (!current || !target) return null
  if (compareVersions(from, to) >= 0) return null

  if (target[0] !== current[0]) return 'major'
  if (target[1] !== current[1]) return 'minor'
  return 'patch'
}

function ahead(
  current: string,
  releases: Schemas.ReleaseAvailability[],
): Schemas.ReleaseAvailability[] {
  return releases
    .filter((release) => release.status === 'available')
    .filter((release) => compareVersions(current, release.id.version) < 0)
    .sort((a, b) => compareVersions(b.id.version, a.id.version))
}

/**
 * The version to offer. Deprecated releases are left out: they can still be
 * passed through on the way somewhere else, but nothing should be moved onto
 * one on purpose.
 */
export function nextUpgrade(
  current: string,
  releases: Schemas.ReleaseAvailability[],
): Schemas.ReleaseAvailability | null {
  return ahead(current, releases).find((release) => release.eligible) ?? null
}

/**
 * A newer version that exists but is not being offered to this deployment.
 *
 * Worth a sentence rather than a blank: a customer who has read the release
 * notes elsewhere will otherwise conclude the screen is broken.
 */
export function heldBack(
  current: string,
  releases: Schemas.ReleaseAvailability[],
): Schemas.ReleaseAvailability | null {
  const offered = nextUpgrade(current, releases)
  const newest = ahead(current, releases).find((release) => !release.eligible)

  if (!newest) return null
  // Something better is already on offer, so a version held back beyond it is
  // not what the customer is missing out on today.
  if (offered && compareVersions(offered.id.version, newest.id.version) >= 0) return null

  return newest
}

export function whyHeldBack(reason: Schemas.IneligibilityReason | null | undefined): string {
  if (!reason) return 'This version is not being offered to this deployment yet.'

  switch (reason.kind) {
    case 'outside_rollout':
      return 'This version is still being rolled out. It will be offered here when it reaches you.'
    case 'operator_too_old':
      return `The cluster this runs on needs to be updated first. It is on ${
        reason.dataplane ?? 'an older version'
      } and this release needs ${reason.minimum}.`
    case 'not_installable':
      return 'This version is not available to install.'
  }
}
