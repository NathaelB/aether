import type { Schemas } from '@/api/api.client'

/**
 * Turning one trail entry into the line somebody reads during an incident.
 *
 * Pure, and tested on its own, because this is where the entry stops being a
 * record and starts being a claim about what happened. A wrong verb here is a
 * screen that says a cluster was disabled when it was drained — and the whole
 * reason the trail exists is that nobody should have to go and check.
 */

/** What an entry says, in the four parts the screen lays out. */
export interface TrailLine {
  /** What was done, in words rather than in the wire's name. */
  action: string
  /** What it was done to. */
  target: string
  /** Who did it. */
  actor: string
  /** The change, when there is one worth a second line. */
  detail?: string
}

const ACTIONS: Record<Schemas.FleetAuditAction, string> = {
  'dataplane.registered': 'Registered a data plane',
  'dataplane.drained': 'Drained a data plane',
  'dataplane.disabled': 'Disabled a data plane',
  'dataplane.returned_to_service': 'Returned a data plane to service',
  'dataplane.credential_reissued': 'Re-issued a data plane credential',
  'operator.granted': 'Granted platform rights',
  'operator.revoked': 'Revoked platform rights',
}

/**
 * Whether the act took something offline.
 *
 * The two the issue behind this screen names: disabling a cluster and
 * re-issuing its credential each stop a customer's deployment being served
 * without anybody touching the deployment. They are marked so that somebody
 * scanning a week of entries for the cause of an outage finds them without
 * reading every line.
 */
export function interrupts(action: Schemas.FleetAuditAction): boolean {
  return action === 'dataplane.disabled' || action === 'dataplane.credential_reissued'
}

/**
 * A cluster shown by its first segment, a subject in full.
 *
 * A UUID truncated to eight characters is what the data planes screen already
 * shows and what somebody recognises. A subject is not ours to shorten: it is
 * a name the identity provider issued, and half of one identifies nobody.
 */
function describeTarget(target: Schemas.FleetTarget): string {
  return target.kind === 'data_plane' ? target.id.slice(0, 8) : target.subject
}

function describeActor(actor: Schemas.FleetActor): string {
  switch (actor.kind) {
    case 'operator':
      return actor.subject
    case 'api':
      return actor.client_id
    case 'system':
      return 'the platform'
  }
}

/**
 * The change, when it says something the action does not already.
 *
 * A status change reads as an arrow; a rights change lists what was held and
 * what is held now. Anything else is left out rather than dumped as JSON: a
 * line nobody can read is worse than no line, because it still takes the room.
 */
function describeChange(change: Schemas.AuditChange | null | undefined): string | undefined {
  if (!change) return undefined

  const before = change.before as Record<string, unknown>
  const after = change.after as Record<string, unknown>

  if (typeof before?.status === 'string' && typeof after?.status === 'string') {
    return `${before.status} → ${after.status}`
  }

  if (Array.isArray(before?.rights) && Array.isArray(after?.rights)) {
    const held = (rights: unknown[]) => (rights.length === 0 ? 'nothing' : rights.join(', '))

    return `${held(before.rights)} → ${held(after.rights)}`
  }

  return undefined
}

export function toTrailLine(entry: Schemas.FleetAuditEntry): TrailLine {
  return {
    action: ACTIONS[entry.action] ?? entry.action,
    target: describeTarget(entry.target),
    actor: describeActor(entry.actor),
    detail: describeChange(entry.change),
  }
}
