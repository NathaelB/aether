interface ImportMetaEnv {
  readonly VITE_API_URL: string
  readonly VITE_OIDC_ISSUER_URL: string
  readonly VITE_OIDC_CLIENT_ID: string
  // add more environment variables as needed
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}

declare module '*.css'
