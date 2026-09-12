import { UserManager, WebStorageStateStore } from 'oidc-client-ts'
import type { AppConfig } from '@/types/config'

let manager: UserManager | null = null

/**
 * The one user manager the console signs in through.
 *
 * Held here rather than built by `AuthProvider` so the renewal path can reach
 * it: renewing has to be serialised across tabs, and a manager only React can
 * see cannot be coordinated with.
 */
export function createUserManager(config: AppConfig): UserManager {
  manager = new UserManager({
    authority: config.oidc.issuer_url,
    client_id: config.oidc.client_id,
    redirect_uri: window.location.origin + '/',
    post_logout_redirect_uri: window.location.origin + '/',
    scope: 'openid profile email',

    // Survives the tab, which is the whole point: a refresh token that dies
    // with `sessionStorage` sends somebody back to a login form while it is
    // still perfectly valid. What makes this safe to persist is on the other
    // side -- Ferriskey rotates it on every use and revokes the family when
    // one is replayed -- and the renewal lock, without which two tabs would
    // trip that revocation themselves.
    userStore: new WebStorageStateStore({ store: window.localStorage }),

    // Renewal is driven by `renewSession`, not by the library's own timer:
    // that timer calls `signinSilent` directly, so every tab would exchange
    // the same refresh token at the same instant.
    automaticSilentRenew: false,

    // No `silent_redirect_uri`: the iframe flow it feeds needs `prompt=none`,
    // and Ferriskey answers that with its login page instead of the error the
    // spec asks for. Leaving it out turns a ten second timeout into an
    // immediate failure the caller can act on.

    // Session monitoring stays off, as it already was: the setting the old
    // configuration passed was `monitor_session`, which is not the name of
    // anything, and the realm publishes no `check_session_iframe` for it to
    // have watched.

    revokeTokensOnSignout: true,
  })

  return manager
}

export function getUserManager(): UserManager | null {
  return manager
}
