import { useQuery } from '@tanstack/react-query'
import type { SearchRequest } from '@/domain/logs/search'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

/**
 * One search of an organisation's stored logs.
 *
 * `enabled` is the caller's: the Search tab shares a route with Live, and a
 * request should not go out while the reader is looking at the other one.
 */
export const useSearchLogs = (
  organisationId: string | null,
  request: SearchRequest | null,
  enabled: boolean,
) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/logs/search', {
      path: { organisation_id: organisationId ?? 'current' },
      query: {
        from: request?.from ?? '',
        to: request?.to ?? '',
        level_floor: request?.level_floor ?? '',
        q: request?.q,
        deployment_id: request?.deployment_id,
      },
    }).queryOptions,
    enabled: enabled && !!organisationId && !!accessToken && !!request,
  })
}
