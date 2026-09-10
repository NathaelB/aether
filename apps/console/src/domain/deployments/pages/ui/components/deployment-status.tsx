import type { DeploymentStatus } from '../../../types/deployment'
import { cn } from '@/lib/utils'

/**
 * One vocabulary for deployment status, in one place.
 *
 * This map was written twice, byte for byte, in the overview and the detail
 * page. That was survivable while it was a lookup table -- `Record<DeploymentStatus, …>`
 * is exhaustive, so the compiler catches a missing case in both copies. It
 * stopped being survivable when a status was actually added: `deleting`
 * arrived from the API and had to be added to both, in the same commit, or
 * neither built.
 *
 * Two copies is normally a coincidence rather than a pattern. The thing that
 * makes this one worth extracting is not the count, it is that they must
 * change together and always will.
 */
const STATUS: Record<DeploymentStatus, { label: string; tone: string; dot: string }> = {
  pending: { label: 'Pending', tone: 'text-muted-foreground bg-muted', dot: 'bg-muted-foreground' },
  scheduling: {
    label: 'Scheduling',
    tone: 'text-blue-700 bg-blue-50 dark:text-blue-300 dark:bg-blue-950',
    dot: 'bg-blue-400',
  },
  in_progress: {
    label: 'In Progress',
    tone: 'text-blue-700 bg-blue-50 dark:text-blue-300 dark:bg-blue-950',
    dot: 'bg-blue-500 animate-pulse',
  },
  successful: {
    label: 'Successful',
    tone: 'text-green-700 bg-green-50 dark:text-green-300 dark:bg-green-950',
    dot: 'bg-green-500',
  },
  failed: {
    label: 'Failed',
    tone: 'text-red-700 bg-red-50 dark:text-red-300 dark:bg-red-950',
    dot: 'bg-red-500',
  },
  deleting: {
    label: 'Deleting',
    tone: 'text-red-700 bg-red-50 dark:text-red-300 dark:bg-red-950',
    dot: 'bg-red-400',
  },
  maintenance: {
    label: 'Maintenance',
    tone: 'text-amber-700 bg-amber-50 dark:text-amber-300 dark:bg-amber-950',
    dot: 'bg-amber-500',
  },
  upgrade_required: {
    label: 'Upgrade Required',
    tone: 'text-orange-700 bg-orange-50 dark:text-orange-300 dark:bg-orange-950',
    dot: 'bg-orange-500',
  },
  upgrading: {
    label: 'Upgrading',
    tone: 'text-primary bg-primary/10',
    dot: 'bg-primary animate-pulse',
  },
}

/**
 * Falls back rather than throwing on a status the console does not know.
 *
 * The generated client makes an unknown status a type error at build time, so
 * this can only happen against an API newer than the console -- and showing
 * the raw value beats showing nothing.
 */
function deploymentStatus(status: DeploymentStatus) {
  return STATUS[status] ?? { label: status, tone: 'text-muted-foreground bg-muted', dot: 'bg-muted-foreground' }
}

export function DeploymentStatusBadge({ status }: { status: DeploymentStatus }) {
  const { label, tone, dot } = deploymentStatus(status)

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-md px-2 py-0.5 text-xs font-medium',
        tone,
      )}
    >
      <span className={cn('h-1.5 w-1.5 rounded-full', dot)} aria-hidden='true' />
      {label}
    </span>
  )
}
