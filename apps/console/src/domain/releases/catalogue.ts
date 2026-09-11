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
