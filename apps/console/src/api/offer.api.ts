import { useQuery } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

/**
 * What this organisation may choose from, and what would open the rest.
 *
 * Asked of the platform rather than restated here. Which offers exist, what
 * each gives and which tier opens it are three tables that would drift the
 * first time one of them changed.
 */
export const useGetOffers = (organisationId: string | null) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/offers', {
      path: { organisation_id: organisationId ?? 'current' },
    }).queryOptions,
    enabled: !!organisationId && !!accessToken,
  })
}
