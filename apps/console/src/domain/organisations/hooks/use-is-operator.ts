import { selectAccessToken, useAuthStore } from '@/stores/auth'

const OPERATOR_ROLE = 'aether-operator'

/**
 * Read from the access token rather than the ID token: realm roles travel in
 * the access token, which is also what the API checks.
 */
export function useIsOperator(): boolean {
  const accessToken = useAuthStore(selectAccessToken)

  if (!accessToken) return false

  try {
    const payload = accessToken.split('.')[1]
    const claims = JSON.parse(atob(payload.replace(/-/g, '+').replace(/_/g, '/')))
    return Boolean(claims?.realm_access?.roles?.includes(OPERATOR_ROLE))
  } catch {
    return false
  }
}
