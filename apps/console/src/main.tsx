import { StrictMode } from 'react'
import ReactDOM from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { router } from './router'
import './index.css'
import { ThemeProvider } from './components/theme-provider'
import { ApiRequestError } from './api/api.fetch'
import { TanstackQueryApiClient } from './api/api.tanstack'

/**
 * Retries only what could answer differently.
 *
 * The default retries three times whatever the failure, so a screen somebody
 * may not open spent four requests being told no, and only then drew its
 * empty state -- which read as a slow page rather than a closed door.
 */
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (failureCount, error) => {
        if (error instanceof ApiRequestError) return error.worthRetrying && failureCount < 3

        return failureCount < 3
      },
    },
  },
})

declare global {
  interface Window {
    api: TanstackQueryApiClient
    apiUrl: string
    issuerUrl?: string
    oidcConfiguration?: {
      client_id: string
      redirect_uri: string
      scope: string
      authority: string
      silent_redirect_uri?: string
      monitor_session?: boolean
      automaticSilentRenew?: boolean
      onSigninCallback?: () => void
    }
    inDevelopmentMode: boolean
  }
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <ThemeProvider defaultTheme='dark' storageKey='aether-ui-theme'>
        <RouterProvider router={router} />
      </ThemeProvider>
    </QueryClientProvider>
  </StrictMode>
)
