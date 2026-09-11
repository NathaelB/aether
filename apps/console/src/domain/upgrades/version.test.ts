import { describe, expect, it } from 'vitest'
import type { Schemas } from '@/api/api.client'
import { changeBetween, compareVersions, heldBack, nextUpgrade, whyHeldBack } from './version'

function release(
  version: string,
  status: Schemas.ReleaseStatus = 'available',
  eligible = true,
  reason: Schemas.IneligibilityReason | null = null,
): Schemas.ReleaseAvailability {
  return {
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    id: { kind: 'ferriskey', version },
    notes: 'notes',
    risk: 'none',
    status,
    steps_through: [],
    rollout: { percentage: 100, pilot_organisations: [] },
    eligible,
    reason,
  }
}

describe('compareVersions', () => {
  /** String order puts 10 before 9, which is how a customer gets offered a downgrade. */
  it('orders by number and not by text', () => {
    expect(compareVersions('26.0.9', '26.0.10')).toBeLessThan(0)
    expect(compareVersions('9.0.0', '10.0.0')).toBeLessThan(0)
    expect(compareVersions('26.1.0', '26.1.0')).toBe(0)
  })
})

describe('changeBetween', () => {
  it('names the part of the version that moved', () => {
    expect(changeBetween('26.0.0', '26.0.1')).toBe('patch')
    expect(changeBetween('26.0.0', '26.1.0')).toBe('minor')
    expect(changeBetween('26.0.0', '27.0.0')).toBe('major')
  })

  /**
   * A jump that moves the minor and the patch at once is still the bigger of
   * the two. Reporting the patch would tell a customer the change is smaller
   * than it is.
   */
  it('reports the largest part that moved', () => {
    expect(changeBetween('26.0.0', '26.1.3')).toBe('minor')
    expect(changeBetween('26.0.0', '27.2.1')).toBe('major')
  })

  it('refuses anything that is not ahead', () => {
    expect(changeBetween('26.1.0', '26.0.0')).toBeNull()
    expect(changeBetween('26.1.0', '26.1.0')).toBeNull()
  })

  it('says nothing about a version it cannot read', () => {
    expect(changeBetween('26.0.0', 'latest')).toBeNull()
    expect(changeBetween('nightly', '26.0.0')).toBeNull()
  })
})

describe('nextUpgrade', () => {
  it('offers the newest version that can be installed', () => {
    const offered = nextUpgrade('26.0.0', [
      release('26.0.1'),
      release('26.2.0'),
      release('26.1.0'),
    ])

    expect(offered?.id.version).toBe('26.2.0')
  })

  /**
   * A deprecated version can be passed through on the way somewhere else, but
   * moving onto one on purpose buys an upgrade that is already on its way out.
   */
  it('never offers a version that is not available', () => {
    const offered = nextUpgrade('26.0.0', [
      release('26.1.0', 'deprecated'),
      release('26.2.0', 'upcoming'),
      release('26.3.0', 'withdrawn'),
    ])

    expect(offered).toBeNull()
  })

  it('offers nothing when the deployment is already on the newest', () => {
    expect(nextUpgrade('26.2.0', [release('26.1.0'), release('26.2.0')])).toBeNull()
  })

  /**
   * A release is rolled out gradually, so one that exists is not necessarily
   * one this deployment may have.
   */
  it('never offers a version this deployment is not eligible for', () => {
    const offered = nextUpgrade('26.0.0', [
      release('26.2.0', 'available', false, { kind: 'outside_rollout' }),
      release('26.1.0'),
    ])

    expect(offered?.id.version).toBe('26.1.0')
  })
})

describe('heldBack', () => {
  /**
   * Worth a sentence rather than a blank. A customer who read the release
   * notes elsewhere would otherwise conclude the screen is broken.
   */
  it('names a newer version that exists but is not on offer', () => {
    const held = heldBack('26.0.0', [
      release('26.2.0', 'available', false, { kind: 'outside_rollout' }),
      release('26.1.0'),
    ])

    expect(held?.id.version).toBe('26.2.0')
  })

  it('says nothing when everything ahead is on offer', () => {
    expect(heldBack('26.0.0', [release('26.1.0'), release('26.2.0')])).toBeNull()
  })

  it('says nothing when there is nothing ahead at all', () => {
    expect(heldBack('26.2.0', [release('26.1.0'), release('26.2.0')])).toBeNull()
  })
})

describe('whyHeldBack', () => {
  it('explains a rollout in terms of what happens next', () => {
    expect(whyHeldBack({ kind: 'outside_rollout' })).toContain('rolled out')
  })

  /** The version that has to move is the cluster's, not the customer's. */
  it('names both operator versions when the cluster is behind', () => {
    const reason = whyHeldBack({
      kind: 'operator_too_old',
      minimum: '1.4.0',
      dataplane: '1.2.0',
    })

    expect(reason).toContain('1.4.0')
    expect(reason).toContain('1.2.0')
  })

  it('falls back to a plain sentence when no reason travelled', () => {
    expect(whyHeldBack(null)).toContain('not being offered')
  })
})
