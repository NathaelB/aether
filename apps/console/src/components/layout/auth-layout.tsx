import { PropsWithChildren, useEffect, useRef } from 'react'
import { useAuth } from 'react-oidc-context'
import { PageLoader } from '../ui/page-loader'
import { useAuthStore } from '@/stores/auth'
import { OrganisationsBootstrap } from '@/domain/organisations/pages/feature/organisations-bootstrap'

export function AuthLayout({ children }: PropsWithChildren) {
  const { isAuthenticated, isLoading, signinRedirect, user, error } = useAuth()
  const { setAccessToken, setProfile, setUser, clear } = useAuthStore()
  const isSilentCallback = window.location.pathname === '/authentication/silent-callback'
  const shouldForceLoginPrompt = window.sessionStorage.getItem('aether:force_login_prompt') === '1'
  const hasTriggeredRedirect = useRef(false)

  useEffect(() => {
    if (isAuthenticated) {
      hasTriggeredRedirect.current = false
    }
  }, [isAuthenticated])

  useEffect(() => {
    if (!isSilentCallback && !isAuthenticated && !isLoading && !hasTriggeredRedirect.current) {
      hasTriggeredRedirect.current = true
      clear()
      if (shouldForceLoginPrompt) {
        window.sessionStorage.removeItem('aether:force_login_prompt')
        signinRedirect({
          extraQueryParams: {
            prompt: 'login',
            max_age: '0',
          },
        })
        return
      }

      signinRedirect()
    }
  }, [isSilentCallback, isAuthenticated, signinRedirect, isLoading, clear, shouldForceLoginPrompt])

  useEffect(() => {
    console.log('auth state changed', { user, token: user?.access_token })
    if (user?.access_token) {
      if (user.profile) {
        console.log('setting profile', user.profile)
        setProfile(user.profile)
      }
      setUser(user)
      setAccessToken(user.access_token)
    }
  }, [user, setProfile, setUser, setAccessToken])

  useEffect(() => {
    if (!user || isSilentCallback) {
      return
    }

    const checkTokenValidity = () => {
      if (!user.expired || hasTriggeredRedirect.current) {
        return
      }

      hasTriggeredRedirect.current = true
      clear()
      signinRedirect()
    }

    checkTokenValidity()
    const interval = window.setInterval(checkTokenValidity, 5000)

    return () => {
      window.clearInterval(interval)
    }
  }, [user, isSilentCallback, clear, signinRedirect])

  if (error) {
    return (
      <div className='w-full h-screen flex items-center justify-center p-6'>
        <div className='max-w-md text-center space-y-4'>
          <p className='text-sm text-muted-foreground'>
            Authentication error: {error.message}
          </p>
          <button
            type='button'
            className='px-4 py-2 rounded-md bg-primary text-primary-foreground'
            onClick={() => signinRedirect()}
          >
            Retry login
          </button>
        </div>
      </div>
    )
  }

  if (isSilentCallback || !isAuthenticated || isLoading) {
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
