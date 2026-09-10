import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import type { Schemas } from '@/api/api.client'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

export const useGetReleasesForOperator = (kind: Schemas.DeploymentKind) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/releases/operator/{kind}', { path: { kind } }).queryOptions,
    enabled: !!accessToken,
  })
}

export const useMoveRelease = (kind: Schemas.DeploymentKind) => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation('put', '/releases/operator/{kind}/{version}/status').mutationOptions,
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: window.api.get('/releases/operator/{kind}', { path: { kind } }).queryKey,
      })
    },
  })
}
