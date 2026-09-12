import { PropsWithChildren, useCallback, useEffect, useRef } from 'react'
import { useAuth } from 'react-oidc-context'
import { PageLoader } from '../ui/page-loader'
import { useAuthStore } from '@/stores/auth'
import { OrganisationsBootstrap } from '@/domain/organisations/pages/feature/organisations-bootstrap'
import { getUserManager } from '@/lib/auth/user-manager'
import { renewSession } from '@/lib/auth/session'
import { renewIn, renewalFor } from '@/lib/auth/renewal'

/**
 * Floor on how often a session is renewed.
 *
 * A realm may hand out access tokens shorter than the renewal margin, and
 * without a floor that arms the next renewal for right now, over and over.
 */
const MINIMUM_RENEWAL_INTERVAL_MS = 5_000

export function AuthLayout({ children }: PropsWithChildren) {
  const { isAuthenticated, isLoading, signinRedirect, user, error } = useAuth()
  const { setAccessToken, setProfile, setUser, clear } = useAuthStore()
  const isRedirecting = useRef(false)

  /**
   * Gets the session back, and only asks the person to sign in again when it
   * cannot.
   *
   * An expired access token is not an expired session: the refresh token
   * outlives it, and trading it costs one request against a round trip to the
   * identity provider and a login form.
   */
  const resume = useCallback(async () => {
    const manager = getUserManager()
    if (!manager || isRedirecting.current) return

    try {
      if (await renewSession(manager)) return
    } catch {
      // The exchange was refused -- a revoked token, a session ended
      // elsewhere. Signing in again is the only way forward.
    }

    isRedirecting.current = true
    clear()
    await signinRedirect()
  }, [clear, signinRedirect])

  useEffect(() => {
    if (isAuthenticated) {
      isRedirecting.current = false
    }
  }, [isAuthenticated])

  useEffect(() => {
    if (isAuthenticated || isLoading) return

    void resume()
  }, [isAuthenticated, isLoading, resume])

  useEffect(() => {
    if (user?.access_token) {
      if (user.profile) {
        setProfile(user.profile)
      }
      setUser(user)
      setAccessToken(user.access_token)
    }
  }, [user, setProfile, setUser, setAccessToken])

  // Renewal is armed from the session's own expiry rather than polled. A
  // backgrounded tab has its timers throttled, so the moment of return is
  // also checked: that is exactly when a session is found to have expired
  // unattended.
  useEffect(() => {
    // Only a live session is kept alive here. Reviving a dead one belongs to
    // the effect above, and letting both own it turns a session the identity
    // provider keeps handing back expired into a renewal loop.
    if (!user || !isAuthenticated) return

    const session = { expiresAt: user.expires_at, refreshToken: user.refresh_token }

    const timer = window.setTimeout(
      () => void resume(),
      Math.max(renewIn(session, epochSeconds()), MINIMUM_RENEWAL_INTERVAL_MS)
    )

    const resumeIfDue = () => {
      if (document.visibilityState === 'hidden') return
      if (renewalFor(session, epochSeconds()) === 'adopt') return

      void resume()
    }

    document.addEventListener('visibilitychange', resumeIfDue)
    window.addEventListener('focus', resumeIfDue)

    return () => {
      window.clearTimeout(timer)
      document.removeEventListener('visibilitychange', resumeIfDue)
      window.removeEventListener('focus', resumeIfDue)
    }
  }, [user, isAuthenticated, resume])

  // Authentication failing is not a destination. Whatever went wrong, the way
  // out is another attempt, and the stored session goes first so a poisoned
  // one cannot refuse the same way forever.
  if (error && !isAuthenticated) {
    return (
      <div className='w-full h-screen flex items-center justify-center p-6'>
        <div className='max-w-md text-center space-y-4'>
          <p className='text-sm text-muted-foreground'>Authentication error: {error.message}</p>
          <button
            type='button'
            className='px-4 py-2 rounded-md bg-primary text-primary-foreground'
            onClick={() => {
              void (async () => {
                await getUserManager()?.removeUser()
                clear()
                await signinRedirect()
              })()
            }}
          >
            Sign in again
          </button>
        </div>
      </div>
    )
  }

  if (!isAuthenticated || isLoading) {
    return (
      <div className='w-full h-screen'>
        <PageLoader />
      </div>
    )
  }

  return (
    <div>
      <OrganisationsBootstrap />
      {children}
    </div>
  )
}

function epochSeconds(): number {
  return Math.floor(Date.now() / 1000)
}
