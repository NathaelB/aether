import { describe, expect, it } from 'vitest'
import type { Schemas } from '@/api/api.client'
import { interrupts, toTrailLine } from './fleet-trail'

function entry(over: Partial<Schemas.FleetAuditEntry> = {}): Schemas.FleetAuditEntry {
  return {
    id: 'ca5b1e6a-0000-0000-0000-000000000000',
    action: 'dataplane.drained',
    actor: { kind: 'operator', subject: 'operator-1' },
    target: { kind: 'data_plane', id: 'd1e5b0aa-1111-2222-3333-444444444444' },
    change: null,
    recorded_at: '2026-09-20T10:00:00Z',
    ...over,
  }
}

describe('what a trail entry says', () => {
  it('names the act in words rather than in the wire name', () => {
    expect(toTrailLine(entry()).action).toBe('Drained a data plane')
    expect(toTrailLine(entry({ action: 'operator.revoked' })).action).toBe(
      'Revoked platform rights',
    )
  })

  /**
   * A cluster is recognised by the first segment of its id, which is what the
   * data planes screen already shows. A subject is a name the identity
   * provider issued and half of one identifies nobody, so it is never cut.
   */
  it('shortens a cluster and leaves a subject whole', () => {
    expect(toTrailLine(entry()).target).toBe('d1e5b0aa')

    const granted = entry({
      action: 'operator.granted',
      target: { kind: 'operator', subject: 'a-very-long-subject-from-the-idp' },
    })
    expect(toTrailLine(granted).target).toBe('a-very-long-subject-from-the-idp')
  })

  it('names the caller a client acted as, and the platform when nobody did', () => {
    expect(toTrailLine(entry({ actor: { kind: 'api', client_id: 'herald-ops' } })).actor).toBe(
      'herald-ops',
    )
    expect(toTrailLine(entry({ actor: { kind: 'system' } })).actor).toBe('the platform')
  })

  it('reads a status change as an arrow', () => {
    const drained = entry({
      change: { before: { status: 'active' }, after: { status: 'draining' } },
    })

    expect(toTrailLine(drained).detail).toBe('active → draining')
  })

  /**
   * An empty set has to read as something. Left as `→ ` it would look like the
   * screen failed to render rather than like a revocation.
   */
  it('reads a rights change, with nothing named as nothing', () => {
    const revoked = entry({
      action: 'operator.revoked',
      change: { before: { rights: ['view_estate', 'operate_fleet'] }, after: { rights: [] } },
    })

    expect(toTrailLine(revoked).detail).toBe('view_estate, operate_fleet → nothing')
  })

  /**
   * A shape nobody wrote a reading for is left out rather than dumped as
   * JSON. It still takes the room, and nobody can read it.
   */
  it('says nothing about a change it has no reading for', () => {
    expect(toTrailLine(entry()).detail).toBeUndefined()
    expect(toTrailLine(entry({ change: { before: { a: 1 }, after: { a: 2 } } })).detail).toBeUndefined()
  })
})

describe('which acts took something offline', () => {
  /**
   * The two the trail exists for: either one stops a customer's deployment
   * being served without anybody touching the deployment.
   */
  it('marks disabling a cluster and re-issuing its credential', () => {
    expect(interrupts('dataplane.disabled')).toBe(true)
    expect(interrupts('dataplane.credential_reissued')).toBe(true)
  })

  it('leaves the rest unmarked, including draining', () => {
    expect(interrupts('dataplane.drained')).toBe(false)
    expect(interrupts('dataplane.registered')).toBe(false)
    expect(interrupts('operator.granted')).toBe(false)
  })
})
