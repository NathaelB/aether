import { PropsWithChildren, useEffect } from 'react'
import { useAuth } from 'react-oidc-context'
import { PageLoader } from '../ui/page-loader'
import { useAuthStore } from '@/stores/auth'

export function AuthLayout({ children }: PropsWithChildren) {
  const { isAuthenticated, isLoading, signinRedirect, user } = useAuth()
  const { setAccessToken, setProfile, setUser } = useAuthStore()

  useEffect(() => {
    if (!isAuthenticated && !isLoading) {
      signinRedirect()
    }
  }, [isAuthenticated, signinRedirect, isLoading])

  useEffect(() => {
    if (user && user.access_token && user.profile) {
      setProfile(user.profile)
      setUser(user)
      setAccessToken(user.access_token)
    }
  }, [user, setProfile, setUser, setAccessToken])

  if (!isAuthenticated || isLoading) {
    return (
      <div className='w-full h-screen'>
        <PageLoader />
      </div>
    )
  }

  return (
    <div>
      {children}
    </div>
  )
}
