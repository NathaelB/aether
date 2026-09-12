import type { User } from 'oidc-client-ts'
import { renewalFor } from './renewal'

/**
 * What renewing a session needs from an OIDC user manager.
 *
 * Narrower than `UserManager`, which it is satisfied by: the point is that a
 * test can hand this three functions instead of a browser.
 */
export interface Renewable {
  getUser(): Promise<User | null>
  signinSilent(): Promise<User | null>
  events: { load(user: User): Promise<void> }
}

/** The subset of the Web Locks API this needs, so a test can stand in for it. */
export interface LockManagerLike {
  request<T>(name: string, work: () => Promise<T>): Promise<T>
}

const RENEWAL_LOCK = 'aether:session-renewal'

/**
 * Renews the session, never more than once at a time across every tab.
 *
 * Ferriskey rotates the refresh token on each use and revokes the whole
 * family when one is presented twice -- so two tabs renewing together do not
 * merely race, they sign the account out everywhere. The lock is what makes a
 * refresh token shared between tabs safe to keep at all.
 *
 * Returns null when there is nothing left to renew with; throws when the
 * exchange itself was refused.
 */
export async function renewSession(
  manager: Renewable,
  now: () => number = epochSeconds,
  locks: LockManagerLike | undefined = globalThis.navigator?.locks
): Promise<User | null> {
  return exclusively(RENEWAL_LOCK, locks, async () => {
    const stored = await manager.getUser()

    switch (renewalFor(sessionOf(stored), now())) {
      case 'adopt':
        // Another tab renewed while this one waited for the lock. Its result
        // is already in storage; raising the event is what tells this tab.
        await manager.events.load(stored as User)
        return stored
      case 'exchange':
        return await manager.signinSilent()
      case 'reauthenticate':
        return null
    }
  })
}

function sessionOf(user: User | null) {
  if (!user) return null

  return { expiresAt: user.expires_at, refreshToken: user.refresh_token }
}

async function exclusively<T>(
  name: string,
  locks: LockManagerLike | undefined,
  work: () => Promise<T>
): Promise<T> {
  // Without the Web Locks API there is no way to serialise across tabs. One
  // tab renewing is still better than none, and the browsers that lack it
  // are the ones this console does not target.
  if (!locks) return work()

  return await locks.request(name, work)
}

function epochSeconds(): number {
  return Math.floor(Date.now() / 1000)
}
