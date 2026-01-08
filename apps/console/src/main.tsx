import { StrictMode } from 'react'
import ReactDOM from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { router } from './router'
import './index.css'
import { ThemeProvider } from './components/theme-provider'
import { TanstackQueryApiClient } from './api/api.tanstack'

const queryClient = new QueryClient()

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
