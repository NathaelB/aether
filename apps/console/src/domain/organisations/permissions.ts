import type { Schemas } from '@/api/api.client'

export type Role = Schemas.Role
export type Member = Schemas.Member

/**
 * The permissions this console can show, and where each one lives.
 *
 * Mirrors `libs/aether-permission`. The bit is the contract: a label moved to
 * the wrong number grants something nobody asked for, silently, and no
 * request fails to say so.
 *
 * `ADMINISTRATOR` is deliberately absent. It is what the owner holds through
 * the organisation, not something to hand out from a form.
 */
export interface Permission {
  bit: number
  label: string
  description: string
  group: PermissionGroup
}

export type PermissionGroup = 'Organisation' | 'Instances' | 'Members' | 'Roles' | 'Billing'

export const PERMISSIONS: Permission[] = [
  { bit: 0, group: 'Organisation', label: 'View the organisation', description: 'See its name, plan and limits.' },
  { bit: 1, group: 'Organisation', label: 'Manage the organisation', description: 'Change its name and settings.' },

  { bit: 2, group: 'Instances', label: 'View instances', description: 'See which instances exist and how they are.' },
  { bit: 3, group: 'Instances', label: 'Create instances', description: 'Deploy a new identity provider.' },
  { bit: 4, group: 'Instances', label: 'Manage instances', description: 'Change resources, upgrade policy and network access.' },
  { bit: 5, group: 'Instances', label: 'Delete instances', description: 'Tear one down. Not reversible.' },
  { bit: 14, group: 'Instances', label: 'Upgrade instances', description: 'Move an instance to another version, and approve one the policy will not apply on its own.' },
  { bit: 15, group: 'Instances', label: 'Read instance logs', description: 'Logs carry identities, addresses and sometimes tokens.' },

  { bit: 6, group: 'Members', label: 'View members', description: 'See who is in the organisation and what is outstanding.' },
  { bit: 7, group: 'Members', label: 'Invite members', description: 'Issue and revoke invitation links.' },
  { bit: 8, group: 'Members', label: 'Manage members', description: 'Change what a member may do.' },
  { bit: 9, group: 'Members', label: 'Remove members', description: 'Put somebody out. Getting back in needs another invitation.' },

  { bit: 10, group: 'Roles', label: 'View roles', description: 'See the roles this organisation defines.' },
  { bit: 11, group: 'Roles', label: 'Manage roles', description: 'Create, change and delete roles.' },

  { bit: 12, group: 'Billing', label: 'View billing', description: 'See invoices and usage charges.' },
  { bit: 13, group: 'Billing', label: 'Manage billing', description: 'Change the plan and payment details.' },
]

export const GROUPS: PermissionGroup[] = [
  'Instances',
  'Members',
  'Roles',
  'Organisation',
  'Billing',
]

export function permissionsIn(group: PermissionGroup): Permission[] {
  return PERMISSIONS.filter((permission) => permission.group === group)
}

/**
 * Everything the catalogue covers, as a mask.
 *
 * Bits 0 to 15, which fit in the 32 bits JavaScript does bitwise arithmetic
 * in. Anything above that is handled by subtraction below, never by shifting.
 */
const MODELLED = PERMISSIONS.reduce((mask, permission) => mask | (1 << permission.bit), 0)
const MODELLED_WIDTH = 65536

/** Which of the catalogue's permissions a mask holds. */
export function heldBy(mask: number): number[] {
  const modelled = mask % MODELLED_WIDTH

  return PERMISSIONS.filter((permission) => (modelled & (1 << permission.bit)) !== 0).map(
    (permission) => permission.bit,
  )
}

export function holds(mask: number, bit: number): boolean {
  return (Math.floor(mask / 2 ** bit) % 2) === 1
}

/**
 * What a mask carries that this console cannot show.
 *
 * Computed by subtraction rather than by masking: ADMINISTRATOR is bit 63,
 * and JavaScript's bitwise operators work in 32 bits, so `mask & ~MODELLED`
 * would quietly answer zero for the one value it most matters for.
 */
export function unmodelled(mask: number): number {
  const modelled = mask % MODELLED_WIDTH

  return mask - (modelled & MODELLED)
}

/**
 * The mask a set of chosen permissions amounts to, keeping what the form
 * could not show.
 *
 * Dropping the remainder would mean a role losing rights nobody meant to take
 * away, on a save that looks like it changed one checkbox.
 */
export function maskFrom(bits: number[], carriedOver = 0): number {
  const chosen = bits
    .filter((bit) => PERMISSIONS.some((permission) => permission.bit === bit))
    .reduce((mask, bit) => mask | (1 << bit), 0)

  return chosen + carriedOver
}

/** How many members hold this role. */
export function heldByCount(role: Role, members: Member[]): number {
  return members.filter((member) => member.roles.some((held) => held.id === role.id)).length
}

/** What deleting this role would take away, said in one line. */
export function describeDeletion(role: Role, members: Member[]): string {
  const count = heldByCount(role, members)

  if (count === 0) {
    return `Nobody holds ${role.name}, so deleting it changes what nobody can do.`
  }

  const who = count === 1 ? 'one member' : `${count} members`

  return `${who} hold ${role.name} and will lose what it grants, immediately and without a new sign-in.`
}

/** What a role permits, in one line. */
export function summarise(role: Role): string {
  const held = heldBy(role.permissions)
  const extra = unmodelled(role.permissions)

  if (held.length === 0 && extra === 0) return 'Nothing'

  const labels = held
    .map((bit) => PERMISSIONS.find((permission) => permission.bit === bit)?.label)
    .filter((label): label is string => !!label)

  if (extra > 0) {
    labels.push('and permissions this console does not know about')
  }

  return labels.join(', ')
}

/** What is wrong with a name somebody typed, or nothing. */
export function checkName(raw: string, existing: Role[], editing?: Role): string | null {
  const value = raw.trim()
  if (value.length === 0) return 'A name is needed.'
  if (value.length > 64) return 'That is longer than 64 characters.'

  const taken = existing.some(
    (role) => role.id !== editing?.id && role.name.toLowerCase() === value.toLowerCase(),
  )

  return taken ? `This organisation already has a role called ${value}.` : null
}

/**
 * The bit the owner holds instead of every other one.
 *
 * `Permissions::can` honours it centrally on the platform, so anything
 * reading a mask here has to do the same or an owner sees a console with
 * every button hidden.
 */
const ADMINISTRATOR = 63

/**
 * Whether the caller may do this, given what they hold here.
 *
 * Never `mask & (1 << bit)`: ADMINISTRATOR is bit 63 and JavaScript does
 * bitwise arithmetic in 32 bits, so the shift wraps to zero and the owner
 * comes out able to do nothing.
 */
export function can(mask: number | undefined, bit: number): boolean {
  if (mask === undefined) return false

  return holds(mask, ADMINISTRATOR) || holds(mask, bit)
}

/** The bits, by name, for the places that gate on one. */
export const CAN = {
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
} as const
