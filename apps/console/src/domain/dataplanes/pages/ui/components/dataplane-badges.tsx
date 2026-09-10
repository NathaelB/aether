import type { Schemas } from '@/api/api.client'
import { StatusBadge, type Tone } from '@/components/ui/status-badge'
import { Globe, Lock } from 'lucide-react'
import { allocationLabel } from '../../../capacity'
import { liveness } from '../../../liveness'

const STATUS_TONES: Record<Schemas.DataPlaneStatus, Tone> = {
  Active: 'success',
  Provisioning: 'progress',
  Draining: 'warning',
  Disabled: 'neutral',
  Failed: 'danger',
}

export function DataPlaneStatusBadge({ status }: { status: Schemas.DataPlaneStatus }) {
  return <StatusBadge tone={STATUS_TONES[status]}>{status}</StatusBadge>
}

export function DataPlaneLivenessBadge({
  dataplane,
}: {
  dataplane: Pick<Schemas.DataPlane, 'last_seen_at'>
}) {
  const value = liveness(dataplane)
  const tone: Tone = value === 'reachable' ? 'success' : value === 'stale' ? 'danger' : 'neutral'
  const label =
    value === 'reachable' ? 'Reporting' : value === 'stale' ? 'Not reporting' : 'Never reported'

  return <StatusBadge tone={tone}>{label}</StatusBadge>
}

export function DataPlaneAllocationBadge({
  allocation,
}: {
  allocation: Schemas.DataPlaneAllocation
}) {
  const dedicated = allocationLabel(allocation) === 'Dedicated'

  return (
    <StatusBadge
      tone={dedicated ? 'accent' : 'neutral'}
      dot={false}
      icon={dedicated ? <Lock className='h-3 w-3' /> : <Globe className='h-3 w-3' />}
    >
      {dedicated ? 'Dedicated' : 'Shared'}
    </StatusBadge>
  )
}
