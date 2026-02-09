import { AppConfig, RuntimeConfig } from '@/types/config'

async function loadConfigFromFile(): Promise<RuntimeConfig | null> {
  try {
    const response = await fetch('/config.json', {
      cache: 'no-cache',
      headers: {
        'Content-Type': 'application/json',
      },
    })

    if (!response.ok) {
      console.warn('config.json not found, falling back to environment variables')
      return null
    }

    const config = await response.json()

    if (!config.oidc_issuer_url || !config.oidc_client_id) {
      console.warn('config.json is missing required fields, falling back to environment variables')
      return null
    }
    console.log('Loaded configuration from config.json:', config)
    return {
      IN_DEVELOPMENT_MODE: false,
      OIDC_ISSUER_URL: config.oidc_issuer_url,
      OIDC_CLIENT_ID: config.oidc_client_id,
      API_URL: config.api_url,
    } as RuntimeConfig
  } catch (error) {
    console.warn('Failed to load config.json:', error)
    return null
  }
}

function loadConfigFromEnv(): RuntimeConfig | null {
  const issuerUrl = import.meta.env.VITE_OIDC_ISSUER_URL
  const clientId = import.meta.env.VITE_OIDC_CLIENT_ID
  const apiUrl = import.meta.env.VITE_API_URL

  console.log('Loaded configuration from environment variables:', {
    OIDC_ISSUER_URL: issuerUrl,
    OIDC_CLIENT_ID: clientId,
    API_URL: apiUrl,
  })

  if (!issuerUrl || !clientId) {
    return null
  }

  return {
    OIDC_ISSUER_URL: issuerUrl,
    OIDC_CLIENT_ID: clientId,
    API_URL: apiUrl,
    IN_DEVELOPMENT_MODE: true,
  }
}

export async function loadAppConfig(): Promise<AppConfig> {
  let runtimeConfig = await loadConfigFromFile()

  if (!runtimeConfig) {
    console.log('Falling back to environment variables for configuration')
    runtimeConfig = loadConfigFromEnv()
  }

  // Validate configuration
  if (!runtimeConfig || !runtimeConfig.OIDC_ISSUER_URL || !runtimeConfig.OIDC_CLIENT_ID) {
    throw new Error(
      'Missing required configuration. Please ensure OIDC_ISSUER_URL and OIDC_CLIENT_ID are set in config.json or environment variables.'
    )
  }

  const config: AppConfig = {
    oidc: {
      issuer_url: runtimeConfig.OIDC_ISSUER_URL,
      client_id: runtimeConfig.OIDC_CLIENT_ID,
    },
    api_url: runtimeConfig.API_URL,
    in_development_mode: runtimeConfig.IN_DEVELOPMENT_MODE,
  }

  return config
}

export function setupOidcConfiguration(config: AppConfig): void {
  window.issuerUrl = config.oidc.issuer_url
  window.oidcConfiguration = {
    client_id: config.oidc.client_id,
    redirect_uri: window.location.origin + '/',
    silent_redirect_uri: window.location.origin + '/authentication/silent-callback',
    scope: 'openid profile email',
    authority: config.oidc.issuer_url,
    monitor_session: true,
    automaticSilentRenew: true,
    onSigninCallback: () => {
      window.history.replaceState({}, document.title, window.location.pathname)
    },
  }
  window.inDevelopmentMode = config.in_development_mode
}

export async function initializeAppConfig(): Promise<{
  config: AppConfig | null
  error: string | null
}> {
  try {
    const config = await loadAppConfig()
    setupOidcConfiguration(config)

    return {
      config,
      error: null,
    }
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : 'Unknown configuration error'
    console.error('Failed to initialize app configuration:', errorMessage)

    return {
      config: null,
      error: errorMessage,
    }
  }
}
