import { useEffect, useState } from 'react'
import { SetupAppLayout } from './setup-app-layout'
import { initializeAppConfig } from '@/lib/config-app'
import { createApiClient } from '@/api/api.client'
import { fetcher } from '@/api/api.fetch'
import { TanstackQueryApiClient } from '@/api/api.tanstack'
import { Outlet } from '@tanstack/react-router'

export function AppShell() {
  const [isConfiguring, setIsConfiguring] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    async function setupApp() {
      setIsConfiguring(true)

      const { error: configError, config } = await initializeAppConfig()

      if (configError) {
        setError(configError)
      }

      if (config && config.api_url) {
        const api = createApiClient(fetcher, config.api_url)

        window.api = new TanstackQueryApiClient(api)
        window.apiUrl = config.api_url
      }

      setIsConfiguring(false)
    }

    setupApp()
  }, [])

  return (
    <SetupAppLayout isConfiguring={isConfiguring} error={error}>
      <Outlet />
    </SetupAppLayout>
  )
}
