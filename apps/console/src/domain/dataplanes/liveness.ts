import type { Schemas } from '@/api/api.client'

export type Liveness = 'reachable' | 'stale' | 'never-seen'

/**
 * How long a data plane may go quiet before the console calls it stale.
 *
 * **This is a display heuristic, not the control plane's rule.** The control
 * plane has its own heartbeat window and is the only thing that decides
 * whether a data plane may be placed on; it is configurable, and the console
 * is not told what it is.
 *
 * So this is set deliberately long — several times any plausible server-side
 * window. Erring late means the badge can lag behind a control plane that has
 * already stopped placing, which is a badge that says less than it could. Erring
 * early would mean the console calling a plane dead while the control plane
 * keeps placing deployments on it, and contradicting the system about its own
 * state is the worse failure.
 */
const STALE_AFTER_MS = 5 * 60 * 1000

export function liveness(
  dataplane: Pick<Schemas.DataPlane, 'last_seen_at'>,
  now: Date = new Date(),
): Liveness {
  if (!dataplane.last_seen_at) {
    // Never having reported is a different situation from having stopped: one
    // is a data plane still coming up, the other is one that broke. The domain
    // makes the same distinction (`DataPlaneLiveness::NeverSeen`), and
    // collapsing it here would show a brand-new cluster as a failure.
    return 'never-seen'
  }

  const lastSeen = new Date(dataplane.last_seen_at).getTime()
  if (Number.isNaN(lastSeen)) {
    return 'never-seen'
  }

  return now.getTime() - lastSeen > STALE_AFTER_MS ? 'stale' : 'reachable'
}
