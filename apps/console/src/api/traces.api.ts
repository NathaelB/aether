import { useQuery } from '@tanstack/react-query'
import type { TraceSearchRequest } from '@/domain/traces/search'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

/**
 * One search of an organisation's stored traces -- the counterpart to
 * `logs.api.ts`'s `useSearchLogs`.
 */
export const useSearchTraces = (
  organisationId: string | null,
  request: TraceSearchRequest | null,
  enabled: boolean,
) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/traces/search', {
      path: { organisation_id: organisationId ?? 'current' },
      query: {
        from: request?.from ?? '',
        to: request?.to ?? '',
        service_name: request?.service_name,
        status_code: request?.status_code,
        q: request?.q,
        deployment_id: request?.deployment_id,
      },
    }).queryOptions,
    enabled: enabled && !!organisationId && !!accessToken && !!request,
  })
}

/** Every span of one trace, for the waterfall view. */
export const useGetTrace = (
  organisationId: string | null,
  traceId: string | null,
  enabled: boolean,
) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/traces/{trace_id}', {
      path: { organisation_id: organisationId ?? 'current', trace_id: traceId ?? '' },
    }).queryOptions,
    enabled: enabled && !!organisationId && !!traceId && !!accessToken,
  })
}
