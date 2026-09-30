import type { Schemas } from '@/api/api.client'

const KIND_LABELS: Record<Schemas.SignalKind, string> = {
  dataplane_heartbeat_stale: 'Data plane heartbeat stale',
  deployment_unreachable: 'Deployment unreachable',
  backup_missing: 'Backup missing',
  backup_failed: 'Backup failed',
  drill_overdue: 'Drill overdue',
  action_stuck: 'Action stuck',
}

export function signalKindLabel(kind: Schemas.SignalKind): string {
  return KIND_LABELS[kind]
}

/** The subject shown next to a signal: a cluster by its first segment, a deployment or action in full. */
export function signalSubjectLabel(subject: Schemas.SignalSubject): string {
  return subject.kind === 'dataplane' ? subject.id.slice(0, 8) : subject.id
}
