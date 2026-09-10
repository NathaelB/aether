import type { Schemas } from '@/api/api.client'

/**
 * The regions this installation can actually deploy into.
 *
 * The create form used to offer `us-east-1`, `eu-west-1` and
 * `ap-southeast-1` — three AWS region names, hardcoded, matching nothing. The
 * control plane answers `UnknownRegion` for a region no data plane serves, so
 * on a local stack (whose only data plane is in `local`) every one of the three
 * choices was a guaranteed failure, and the only way to succeed was to pick
 * nothing and let the default apply.
 *
 * Deriving the list from the data planes that exist means the form can only
 * offer what placement can satisfy.
 */
export function servedRegions(dataplanes: Schemas.DataPlane[]): string[] {
  const regions = dataplanes
    // A disabled data plane is one an operator took out of service, and a
    // failed one never came up. Offering either region would put the user back
    // in front of the same failure, one step later.
    .filter((dataplane) => dataplane.status !== 'disabled' && dataplane.status !== 'failed')
    .map((dataplane) => dataplane.region)

  return [...new Set(regions)].sort()
}
