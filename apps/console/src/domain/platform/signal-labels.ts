import type { Schemas } from '@/api/api.client'

const KIND_LABELS: Record<Schemas.SignalKind, string> = {
  'dataplane.heartbeat_stale': 'Data plane heartbeat stale',
  'deployment.unreachable': 'Deployment unreachable',
  'backup.missing': 'Backup missing',
  'backup.failed': 'Backup failed',
  'drill.overdue': 'Drill overdue',
  'action.stuck': 'Action stuck',
}

export function signalKindLabel(kind: Schemas.SignalKind): string {
  return KIND_LABELS[kind]
}

/** The subject shown next to a signal: a cluster by its first segment, a deployment or action in full. */
export function signalSubjectLabel(subject: Schemas.SignalSubject): string {
  return subject.kind === 'dataplane' ? subject.id.slice(0, 8) : subject.id
}
