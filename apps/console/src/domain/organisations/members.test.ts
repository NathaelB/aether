import { describe, expect, it } from 'vitest'
import {
  alreadyHere,
  checkEmail,
  describeRoles,
  expiresIn,
  invitationState,
  isOwner,
  outstanding,
  type Invitation,
  type Member,
} from './members'

const NOW = new Date('2026-09-12T12:00:00Z')

function invitation(overrides: Partial<Invitation> = {}): Invitation {
  return {
    id: 'i1',
    organisation_id: 'o1',
    email: 'colleague@acme.test',
    roles: [],
    expires_at: '2026-09-19T12:00:00Z',
    created_at: '2026-09-12T12:00:00Z',
    ...overrides,
  }
}

function member(overrides: Partial<Member> = {}): Member {
  return {
    id: 'm1',
    organisation_id: 'o1',
    user_id: 'u1',
    email: 'colleague@acme.test',
    name: 'Colleague',
    roles: [],
    joined_at: '2026-09-01T12:00:00Z',
    ...overrides,
  }
}

describe('invitationState', () => {
  it('is pending while the clock has not passed it', () => {
    expect(invitationState(invitation(), NOW)).toBe('pending')
  })

  it('is expired once it has', () => {
    expect(
      invitationState(invitation({ expires_at: '2026-09-11T12:00:00Z' }), NOW),
    ).toBe('expired')
  })

  /**
   * What happened, happened. Reading an accepted invitation as expired would
   * describe a membership that exists as one that never formed.
   */
  it('stays accepted after the expiry it beat', () => {
    const walked = invitation({
      expires_at: '2026-09-11T12:00:00Z',
      accepted_at: '2026-09-10T12:00:00Z',
    })

    expect(invitationState(walked, NOW)).toBe('accepted')
  })

  it('reads as revoked even when it was accepted first', () => {
    const cut = invitation({
      accepted_at: '2026-09-10T12:00:00Z',
      revoked_at: '2026-09-11T12:00:00Z',
    })

    expect(invitationState(cut, NOW)).toBe('revoked')
  })
})

describe('outstanding', () => {
  it('keeps only the ones still worth acting on', () => {
    const live = invitation({ id: 'live' })
    const used = invitation({ id: 'used', accepted_at: '2026-09-10T12:00:00Z' })
    const gone = invitation({ id: 'gone', expires_at: '2026-09-01T12:00:00Z' })

    expect(outstanding([live, used, gone], NOW).map((i) => i.id)).toEqual(['live'])
  })
})

describe('isOwner and describeRoles', () => {
  /**
   * The owner holds everything without a role saying so. Left unsaid, their
   * empty role list reads as somebody nobody has granted anything to.
   */
  it('says the owner holds everything rather than nothing', () => {
    const owner = member({ user_id: 'owner' })

    expect(isOwner(owner, 'owner')).toBe(true)
    expect(describeRoles(owner, 'owner')).toContain('owner')
  })

  it('says nothing yet for a member granted nothing', () => {
    expect(describeRoles(member(), 'owner')).toBe('Nothing yet')
  })

  it('lists what somebody actually holds', () => {
    const held = member({
      roles: [
        { id: 'r1', name: 'viewer', permissions: 4, created_at: '', organisation_id: 'o1' },
        { id: 'r2', name: 'operator', permissions: 8, created_at: '', organisation_id: 'o1' },
      ],
    })

    expect(describeRoles(held, 'owner')).toBe('viewer, operator')
  })

  it('marks nobody as owner when the organisation is not loaded yet', () => {
    expect(isOwner(member({ user_id: 'owner' }), undefined)).toBe(false)
  })
})

describe('expiresIn', () => {
  it('answers the question being asked rather than showing a date', () => {
    expect(expiresIn(invitation(), NOW)).toBe('Expires in 7 days')
    expect(expiresIn(invitation({ expires_at: '2026-09-12T18:00:00Z' }), NOW)).toBe(
      'Expires in 6h',
    )
  })

  it('says how long ago one lapsed', () => {
    expect(expiresIn(invitation({ expires_at: '2026-09-10T12:00:00Z' }), NOW)).toBe(
      'Expired 2 days ago',
    )
  })
})

describe('checkEmail', () => {
  it('accepts an address', () => {
    expect(checkEmail('colleague@acme.test')).toBeNull()
    expect(checkEmail('  colleague@acme.test  ')).toBeNull()
  })

  it('refuses what is not one, naming it', () => {
    expect(checkEmail('')).toContain('needed')
    expect(checkEmail('nobody')).toContain('nobody')
    expect(checkEmail('a@b@c')).not.toBeNull()
    expect(checkEmail('a b@acme.test')).not.toBeNull()
  })
})

describe('alreadyHere', () => {
  /**
   * The platform refuses both. Finding that out after filling in a form is
   * worse than being told while the box is still open.
   */
  it('says when the address is already a member', () => {
    expect(alreadyHere('Colleague@Acme.Test', [member()], [], NOW)).toContain('already a member')
  })

  it('says when a link is already waiting for it', () => {
    expect(alreadyHere('colleague@acme.test', [], [invitation()], NOW)).toContain('waiting')
  })

  /**
   * A link that lapsed or was cut off is not in the way: issuing another is
   * exactly what somebody should do next.
   */
  it('does not count an invitation that is no longer outstanding', () => {
    const spent = invitation({ accepted_at: '2026-09-10T12:00:00Z' })

    expect(alreadyHere('colleague@acme.test', [], [spent], NOW)).toBeNull()
  })

  it('says nothing about an empty box', () => {
    expect(alreadyHere('', [member()], [], NOW)).toBeNull()
  })
})
