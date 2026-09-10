import { StatusBadge, type Tone } from '@/components/ui/status-badge'
import type { DeploymentStatus } from '../../../types/deployment'

const STATUS: Record<DeploymentStatus, { label: string; tone: Tone }> = {
  pending: { label: 'Pending', tone: 'neutral' },
  scheduling: { label: 'Scheduling', tone: 'progress' },
  in_progress: { label: 'Deploying', tone: 'progress' },
  successful: { label: 'Running', tone: 'success' },
  failed: { label: 'Failed', tone: 'danger' },
  deleting: { label: 'Deleting', tone: 'warning' },
  deleted: { label: 'Deleted', tone: 'neutral' },
  maintenance: { label: 'Maintenance', tone: 'warning' },
  upgrade_required: { label: 'Upgrade required', tone: 'warning' },
  upgrading: { label: 'Upgrading', tone: 'progress' },
}

export function DeploymentStatusBadge({ status }: { status: DeploymentStatus }) {
  const { label, tone } = STATUS[status] ?? { label: status, tone: 'neutral' as Tone }

  return <StatusBadge tone={tone}>{label}</StatusBadge>
}
