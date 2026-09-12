import type { Schemas } from '@/api/api.client'

export type Member = Schemas.Member
export type Invitation = Schemas.Invitation
export type Role = Schemas.Role

/**
 * Where an invitation stands, now.
 *
 * The same precedence the platform applies, restated here because the API
 * sends three timestamps rather than a state: revoked beats accepted beats
 * expired. An invitation somebody already walked through stays accepted once
 * the clock passes its expiry -- reading it as expired would describe a
 * membership that exists as one that never formed.
 */
export type InvitationState = 'pending' | 'accepted' | 'revoked' | 'expired'

export function invitationState(invitation: Invitation, now: Date): InvitationState {
  if (invitation.revoked_at) return 'revoked'
  if (invitation.accepted_at) return 'accepted'
  if (now.getTime() >= new Date(invitation.expires_at).getTime()) return 'expired'

  return 'pending'
}

/** The ones still worth acting on. */
export function outstanding(invitations: Invitation[], now: Date): Invitation[] {
  return invitations.filter((invitation) => invitationState(invitation, now) === 'pending')
}

/**
 * Whether this member is the person the organisation belongs to.
 *
 * They hold everything without a role saying so, and they cannot be removed.
 * The screen has to say both, or their empty role list reads as somebody
 * nobody has got round to granting anything.
 */
export function isOwner(member: Member, ownerId: string | undefined): boolean {
  return !!ownerId && member.user_id === ownerId
}

/** What a member may do here, in one line. */
export function describeRoles(member: Member, ownerId: string | undefined): string {
  if (isOwner(member, ownerId)) return 'Everything, as the owner'
  if (member.roles.length === 0) return 'Nothing yet'

  return member.roles.map((role) => role.name).join(', ')
}

/**
 * How long a link has left, or how long ago it lapsed.
 *
 * Rounded to days and hours rather than shown as a date: the reader is
 * deciding whether to chase somebody or issue another link, and "in 2 days"
 * answers that where a timestamp does not.
 */
export function expiresIn(invitation: Invitation, now: Date): string {
  const remaining = new Date(invitation.expires_at).getTime() - now.getTime()
  const hours = Math.round(Math.abs(remaining) / 3_600_000)

  const span = hours >= 48 ? `${Math.round(hours / 24)} days` : `${hours}h`

  return remaining >= 0 ? `Expires in ${span}` : `Expired ${span} ago`
}

/** What is wrong with an address somebody typed, or nothing. */
export function checkEmail(raw: string): string | null {
  const value = raw.trim()
  if (value.length === 0) return 'An address is needed.'

  const [local, domain, ...rest] = value.split('@')
  if (rest.length > 0 || !local || !domain || /\s/.test(value)) {
    return `"${value}" is not an address.`
  }

  return null
}

/**
 * Whether this address is already in the organisation, one way or another.
 *
 * The platform refuses both, and finding that out after filling in a form is
 * worse than being told while the box is still open. Matched in one spelling
 * on both sides, as the platform matches it.
 */
export function alreadyHere(
  raw: string,
  members: Member[],
  invitations: Invitation[],
  now: Date,
): string | null {
  const value = raw.trim().toLowerCase()
  if (value.length === 0) return null

  if (members.some((member) => member.email.toLowerCase() === value)) {
    return 'That address is already a member.'
  }

  if (
    outstanding(invitations, now).some(
      (invitation) => invitation.email.toLowerCase() === value,
    )
  ) {
    return 'That address already has an invitation waiting.'
  }

  return null
}

/**
 * The secret out of an invitation link.
 *
 * Read from the query string rather than the path: a path segment ends up in
 * more logs, and the console is the only thing that ever parses this.
 */
export function tokenFromLink(search: string): string | null {
  const token = new URLSearchParams(search).get('token')?.trim()

  return token && token.length > 0 ? token : null
}
