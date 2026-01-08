export interface OidcConfig {
  issuer_url: string
  client_id: string
}

export interface AppConfig {
  oidc: OidcConfig
  api_url?: string
  in_development_mode: boolean
}

export interface RuntimeConfig {
  OIDC_ISSUER_URL: string
  OIDC_CLIENT_ID: string
  API_URL?: string
  IN_DEVELOPMENT_MODE: boolean
}
