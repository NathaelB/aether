import { useQuery } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

export const useGetIamFeatures = (organisationId: string | null) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/features', {
      path: { organisation_id: organisationId ?? 'current' },
    }).queryOptions,
    enabled: !!organisationId && !!accessToken,
  })
}
