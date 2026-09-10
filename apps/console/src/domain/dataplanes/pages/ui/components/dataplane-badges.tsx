import type { Schemas } from '@/api/api.client'
import { cn } from '@/lib/utils'
import { allocationLabel } from '../../../capacity'
import { liveness as computeLiveness, type Liveness } from '../../../liveness'

/**
 * A data plane says two independent things about itself, and showing them as
 * one badge would lose the difference that matters most.
 *
 * `status` is what an operator or a provision decided. `liveness` is whether
 * the cluster is answering. A drained plane is reachable and refuses work; a
 * plane that is Active and silent accepts nothing either. Only both together
 * explain why a deployment is not moving.
 */
const STATUS_STYLES: Record<Schemas.DataPlaneStatus, string> = {
  Active: 'text-green-700 bg-green-50 border-green-200 dark:text-green-300 dark:bg-green-950 dark:border-green-900',
  Provisioning: 'text-blue-700 bg-blue-50 border-blue-200 dark:text-blue-300 dark:bg-blue-950 dark:border-blue-900',
  Draining: 'text-amber-700 bg-amber-50 border-amber-200 dark:text-amber-300 dark:bg-amber-950 dark:border-amber-900',
  Disabled: 'text-muted-foreground bg-muted border-border',
  Failed: 'text-red-700 bg-red-50 border-red-200 dark:text-red-300 dark:bg-red-950 dark:border-red-900',
}

const STATUS_DOTS: Record<Schemas.DataPlaneStatus, string> = {
  Active: 'bg-green-500',
  Provisioning: 'bg-blue-500 animate-pulse',
  Draining: 'bg-amber-500',
  Disabled: 'bg-muted-foreground',
  Failed: 'bg-red-500',
}

const LIVENESS_LABELS: Record<Liveness, string> = {
  reachable: 'Reporting',
  stale: 'Not reporting',
  'never-seen': 'Never reported',
}

const LIVENESS_STYLES: Record<Liveness, string> = {
  reachable: 'text-green-700 bg-green-50 border-green-200 dark:text-green-300 dark:bg-green-950 dark:border-green-900',
  stale: 'text-red-700 bg-red-50 border-red-200 dark:text-red-300 dark:bg-red-950 dark:border-red-900',
  'never-seen': 'text-muted-foreground bg-muted border-border',
}

const badge = 'inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 text-xs font-medium'

export function DataPlaneStatusBadge({ status }: { status: Schemas.DataPlaneStatus }) {
  return (
    <span className={cn(badge, STATUS_STYLES[status])}>
      <span className={cn('h-1.5 w-1.5 rounded-full', STATUS_DOTS[status])} aria-hidden='true' />
      {status}
    </span>
  )
}

export function DataPlaneLivenessBadge({
  dataplane,
}: {
  dataplane: Pick<Schemas.DataPlane, 'last_seen_at'>
}) {
  const value = computeLiveness(dataplane)

  return <span className={cn(badge, LIVENESS_STYLES[value])}>{LIVENESS_LABELS[value]}</span>
}

export function DataPlaneAllocationBadge({
  allocation,
}: {
  allocation: Schemas.DataPlaneAllocation
}) {
  const label = allocationLabel(allocation)

  return (
    <span
      className={cn(
        badge,
        label === 'Dedicated'
          // Dedicated is the exceptional case, so it is the one that gets a
          // colour. Shared is the default and reads as unremarkable, which is
          // what it is.
          ? 'text-primary bg-primary/10 border-primary/30'
          : 'text-muted-foreground bg-muted border-border',
      )}
    >
      {label}
    </span>
  )
}
