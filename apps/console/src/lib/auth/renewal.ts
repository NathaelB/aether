/**
 * When a session is renewed, and with what.
 *
 * Kept apart from the OIDC client so the rule can be read, and tested,
 * without a browser or a user manager.
 */

/**
 * How long before expiry a session is renewed rather than used.
 *
 * A request that leaves with a token about to expire can still arrive after
 * it has, so the margin covers the flight rather than the clock.
 */
export const RENEW_MARGIN_SECONDS = 60

export type Renewal =
  /** Somebody already renewed it. Take what is stored. */
  | 'adopt'
  /** Trade the refresh token for a new session. */
  | 'exchange'
  /** Nothing left to trade with. */
  | 'reauthenticate'

export interface StoredSession {
  /** Epoch seconds, as OIDC stores it. */
  expiresAt?: number
  refreshToken?: string
}

export function renewalFor(session: StoredSession | null | undefined, now: number): Renewal {
  if (!session) return 'reauthenticate'

  if (remainingSeconds(session, now) > RENEW_MARGIN_SECONDS) return 'adopt'

  // An expired access token says nothing about the refresh token: they have
  // their own lifetimes, and the shorter one running out is the ordinary
  // case rather than the end of the session.
  return session.refreshToken ? 'exchange' : 'reauthenticate'
}

/** Milliseconds to wait before renewing, never negative. */
export function renewIn(session: StoredSession | null | undefined, now: number): number {
  if (!session) return 0

  const ahead = remainingSeconds(session, now) - RENEW_MARGIN_SECONDS

  return ahead > 0 ? ahead * 1000 : 0
}

function remainingSeconds(session: StoredSession, now: number): number {
  // A session that does not say when it expires is treated as expired: the
  // alternative is trusting a token nobody can date.
  if (session.expiresAt === undefined) return 0

  return session.expiresAt - now
}
