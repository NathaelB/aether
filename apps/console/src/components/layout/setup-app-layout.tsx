import { PropsWithChildren } from 'react'
import { PageLoader } from '../ui/page-loader'
import { AuthProvider } from 'react-oidc-context'
import { AuthLayout } from './auth-layout'

interface SetupAppLayoutProps extends PropsWithChildren {
  isConfiguring?: boolean
  error?: string | null
}

export function SetupAppLayout({ children, isConfiguring = false, error = null }: SetupAppLayoutProps) {
  if (error) {
    return (
      <div className='h-screen w-screen flex flex-col items-center justify-center'>
        <div className='text-center max-w-md p-8'>
          <div className='w-16 h-16 bg-destructive/10 rounded-full flex items-center justify-center mx-auto mb-4'>
            <svg
              className='w-8 h-8 text-destructive'
              fill='none'
              strokeLinecap='round'
              strokeLinejoin='round'
              strokeWidth='2'
              viewBox='0 0 24 24'
              stroke='currentColor'
            >
              <path d='M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z' />
            </svg>
          </div>
          <h2 className='text-2xl font-bold text-foreground mb-2'>
            Configuration Error
          </h2>
          <p className='text-muted-foreground mb-4'>{error}</p>
          <button
            onClick={() => window.location.reload()}
            className='px-4 py-2 bg-primary text-primary-foreground rounded-md hover:bg-primary/90 transition-colors'
          >
            Retry
          </button>
        </div>
      </div>
    )
  }

  if (isConfiguring || !window.oidcConfiguration) {
    return (
      <div className='h-screen w-screen'>
        <PageLoader />
      </div>
    )
  }

  return (
    <AuthProvider {...window.oidcConfiguration}>
      <AuthLayout>
        {children}
      </AuthLayout>

    </AuthProvider>
  )
}
