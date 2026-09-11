import type { DeploymentStatus } from './types/deployment'

/**
 * The states the platform is still moving a deployment through.
 *
 * A deployment in one of these is being written by a data plane rather than
 * by anyone using the console, so a record read in one of them is already
 * behind by the time it is drawn. Every other state is a resting place: it is
 * reached and then left alone until someone acts, and that act is what
 * refreshes it.
 */
const SETTLING: ReadonlySet<DeploymentStatus> = new Set<DeploymentStatus>([
  'pending',
  'scheduling',
  'in_progress',
  'upgrading',
  'deleting',
])

export function isSettling(status: DeploymentStatus | null | undefined): boolean {
  return !!status && SETTLING.has(status)
}

const SETTLING_INTERVAL_MS = 5_000

/**
 * How long to wait before reading a deployment again, or `false` to stop.
 *
 * Polling ends on the reply that reports a resting state, and that same reply
 * carries the version the upgrade landed on -- which is why nothing has to
 * watch for the moment an upgrade finishes. Without it the page goes on
 * offering the version it has just finished applying until someone reloads.
 */
export function settlingRefetchInterval(
  status: DeploymentStatus | null | undefined,
): number | false {
  return isSettling(status) ? SETTLING_INTERVAL_MS : false
}

/** The same rule over a list: one deployment still moving keeps it fresh. */
export function anySettling(
  deployments: ReadonlyArray<{ status: DeploymentStatus }> | undefined,
): number | false {
  return deployments?.some((deployment) => isSettling(deployment.status))
    ? SETTLING_INTERVAL_MS
    : false
}
