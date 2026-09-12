import { describe, expect, it } from 'vitest'
import {
  CAN,
  GROUPS,
  can,
  PERMISSIONS,
  checkName,
  describeDeletion,
  heldBy,
  heldByCount,
  holds,
  maskFrom,
  permissionsIn,
  summarise,
  unmodelled,
  type Member,
  type Role,
} from './permissions'

/**
 * The bits as `libs/aether-permission` declares them.
 *
 * Written out rather than derived, because deriving them from the catalogue
 * under test would prove only that it agrees with itself. A label that moves
 * to another number grants something nobody asked for, silently, and no
 * request fails to say so -- this is the only place that can catch it.
 */
const AS_THE_PLATFORM_DECLARES_THEM: Record<string, number> = {
  'View the organisation': 0,
  'Manage the organisation': 1,
  'View instances': 2,
  'Create instances': 3,
  'Manage instances': 4,
  'Delete instances': 5,
  'View members': 6,
  'Invite members': 7,
  'Manage members': 8,
  'Remove members': 9,
  'View roles': 10,
  'Manage roles': 11,
  'View billing': 12,
  'Manage billing': 13,
  'Upgrade instances': 14,
  'Read instance logs': 15,
}

function role(overrides: Partial<Role> = {}): Role {
  return {
    id: 'r1',
    name: 'viewer',
    permissions: 4,
    organisation_id: 'o1',
    created_at: '2026-09-12T12:00:00Z',
    ...overrides,
  }
}

function member(roles: Role[]): Member {
  return {
    id: 'm1',
    organisation_id: 'o1',
    user_id: 'u1',
    email: 'somebody@acme.test',
    name: 'Somebody',
    roles,
    joined_at: '2026-09-01T12:00:00Z',
  }
}

describe('the catalogue', () => {
  it('puts every label on the number the platform gave it', () => {
    for (const permission of PERMISSIONS) {
      expect(AS_THE_PLATFORM_DECLARES_THEM[permission.label], permission.label).toBe(
        permission.bit,
      )
    }
  })

  it('covers every permission the platform declares, and no more', () => {
    expect(PERMISSIONS).toHaveLength(Object.keys(AS_THE_PLATFORM_DECLARES_THEM).length)
  })

  it('uses each bit once', () => {
    const bits = PERMISSIONS.map((permission) => permission.bit)

    expect(new Set(bits).size).toBe(bits.length)
  })

  /**
   * It is what the owner holds through the organisation, not something to
   * hand out from a form. Offered as a checkbox it would be handed out.
   */
  it('does not offer ADMINISTRATOR', () => {
    expect(PERMISSIONS.some((permission) => permission.bit === 63)).toBe(false)
    expect(PERMISSIONS.some((permission) => /administrator/i.test(permission.label))).toBe(false)
  })

  it('shows every permission under exactly one group somebody can find', () => {
    const grouped = GROUPS.flatMap((group) => permissionsIn(group))

    expect(grouped).toHaveLength(PERMISSIONS.length)
  })
})

describe('reading a mask and writing it back', () => {
  /**
   * Both directions, for every permission there is. A bit shown against the
   * wrong label and a bit written to the wrong place are the same defect seen
   * from two sides.
   */
  it('round-trips every permission on its own', () => {
    for (const permission of PERMISSIONS) {
      const mask = 2 ** permission.bit

      expect(heldBy(mask), permission.label).toEqual([permission.bit])
      expect(maskFrom([permission.bit]), permission.label).toBe(mask)
      expect(holds(mask, permission.bit), permission.label).toBe(true)
    }
  })

  it('round-trips a mask holding several', () => {
    const mask = maskFrom([2, 6, 15])

    expect(heldBy(mask).sort((a, b) => a - b)).toEqual([2, 6, 15])
  })

  it('holds nothing when the mask is empty', () => {
    expect(heldBy(0)).toEqual([])
    expect(maskFrom([])).toBe(0)
  })

  it('ignores a bit that is not in the catalogue rather than writing it', () => {
    expect(maskFrom([2, 63])).toBe(4)
  })
})

describe('what the console cannot show', () => {
  /**
   * ADMINISTRATOR is bit 63 and JavaScript does bitwise arithmetic in 32
   * bits, so the obvious `mask & ~MODELLED` answers zero for exactly the
   * value where being wrong costs the most.
   */
  it('sees a permission above the ones it models', () => {
    expect(unmodelled(2 ** 63)).toBe(2 ** 63)
    expect(unmodelled(2 ** 63 + 4)).toBe(2 ** 63)
  })

  it('sees nothing extra in a mask it fully understands', () => {
    expect(unmodelled(maskFrom([2, 6, 15]))).toBe(0)
    expect(unmodelled(0)).toBe(0)
  })

  /**
   * The save that would otherwise take away what the form could not show. A
   * role losing everything because somebody ticked one box is not a change
   * anybody asked for.
   */
  it('keeps what it could not show when the mask is written back', () => {
    const original = 2 ** 63 + maskFrom([2])
    const carried = unmodelled(original)

    expect(maskFrom([2, 6], carried)).toBe(2 ** 63 + maskFrom([2, 6]))
  })

  it('says so rather than listing nothing', () => {
    expect(summarise(role({ permissions: 2 ** 63 }))).toContain('does not know about')
  })
})

describe('summarise', () => {
  it('names what a role permits', () => {
    expect(summarise(role({ permissions: maskFrom([2, 6]) }))).toBe(
      'View instances, View members',
    )
  })

  it('says a role permitting nothing permits nothing', () => {
    expect(summarise(role({ permissions: 0 }))).toBe('Nothing')
  })
})

describe('who holds a role', () => {
  it('counts the members holding it', () => {
    const viewer = role({ id: 'viewer' })
    const other = role({ id: 'other' })

    const members = [member([viewer]), member([viewer, other]), member([])]

    expect(heldByCount(viewer, members)).toBe(2)
    expect(heldByCount(other, members)).toBe(1)
  })

  /**
   * Deleting a role held by people takes rights away immediately, with no new
   * sign-in. Saying how many is the difference between a decision and a
   * surprise.
   */
  it('says how many lose what before it is deleted', () => {
    const viewer = role({ id: 'viewer', name: 'viewer' })

    expect(describeDeletion(viewer, [member([viewer])])).toContain('one member')
    expect(describeDeletion(viewer, [member([viewer]), member([viewer])])).toContain('2 members')
    expect(describeDeletion(viewer, [])).toContain('Nobody holds')
  })
})

describe('checkName', () => {
  it('accepts a name nothing else uses', () => {
    expect(checkName('operator', [role({ name: 'viewer' })])).toBeNull()
  })

  it('refuses an empty one', () => {
    expect(checkName('   ', [])).toContain('needed')
  })

  /**
   * Two roles with one name is two things somebody has to tell apart in a
   * list of checkboxes, and nothing in the platform stops it.
   */
  it('refuses a name already taken, whatever the case', () => {
    expect(checkName('Viewer', [role({ name: 'viewer' })])).toContain('already has a role')
  })

  it('lets a role keep its own name while being edited', () => {
    const existing = role({ id: 'r1', name: 'viewer' })

    expect(checkName('viewer', [existing], existing)).toBeNull()
  })
})

describe('can', () => {
  it('answers from the bit the caller holds', () => {
    expect(can(maskFrom([CAN.viewInstances]), CAN.viewInstances)).toBe(true)
    expect(can(maskFrom([CAN.viewInstances]), CAN.createInstances)).toBe(false)
  })

  /**
   * The owner holds ADMINISTRATOR and none of the others. Read literally,
   * they would see a console with every button hidden -- and the platform
   * would let them press all of them.
   */
  it('lets the owner do everything through ADMINISTRATOR alone', () => {
    const owner = 2 ** 63

    for (const bit of Object.values(CAN)) {
      expect(can(owner, bit), String(bit)).toBe(true)
    }
  })

  it('says no while nothing has been read yet', () => {
    expect(can(undefined, CAN.viewInstances)).toBe(false)
  })

  it('says no for somebody holding nothing', () => {
    expect(can(0, CAN.viewInstances)).toBe(false)
  })

  /**
   * Every name in CAN has to be the number the platform declares, for the
   * same reason the catalogue does: a gate on the wrong bit hides the wrong
   * thing, or shows it.
   */
  it('names the bits the platform declares', () => {
    expect(CAN).toEqual({
      viewInstances: 2,
      createInstances: 3,
      manageInstances: 4,
      deleteInstances: 5,
      viewMembers: 6,
      inviteMembers: 7,
      manageMembers: 8,
      removeMembers: 9,
      viewRoles: 10,
      manageRoles: 11,
    })
  })
})
