import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

export const useGetDataplaneUpgrades = () => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/platform/dataplanes/upgrades').queryOptions,
    enabled: !!accessToken,
  })
}

export const useStartDataplaneUpgrade = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation('post', '/platform/dataplanes/upgrade').mutationOptions,
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: window.api.get('/platform/dataplanes/upgrades').queryKey,
      })
    },
  })
}
