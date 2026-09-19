import type { Schemas } from '@/api/api.client'

export type Deployment = Schemas.Deployment

/**
 * Who a deployment could trade hostnames with.
 *
 * Not filtered down to deployments actually related by a restore: the
 * console has no cheap way to know that without asking, and the platform
 * already refuses a pair that is not -- with a reason this screen shows
 * rather than guesses at.
 */
export function otherDeployments(deployments: Deployment[], selfId: string): Deployment[] {
  return deployments.filter((deployment) => deployment.id !== selfId && !deployment.deleted_at)
}

/**
 * What a cutover between these two actually does, in one line.
 *
 * Symmetric and history free on purpose: a cutover is a name swap, true
 * regardless of which of the two was created first or which one currently
 * answers the shared hostname.
 */
export function describeCutover(self: Deployment, other: Deployment): string {
  return `This deployment will take over as "${other.name}", along with the hostname \
deployments answer at that name. "${other.name}" will keep running, renamed to \
"${self.name}", the name this deployment gives up.`
}
