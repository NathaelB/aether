import { useMutation, useQuery } from '@tanstack/react-query'
import { useAuthStore, selectAccessToken } from '@/stores/auth'

export const useGetUserOrganisations = () => {
  const accessToken = useAuthStore(selectAccessToken)
  return useQuery({
    ...window.api.get('/users/@me/organisations').queryOptions,
    enabled: !!accessToken,
  })
}

export const useCreateOrganisation = () => {
  return useMutation({
    ...window.api.mutation('post', '/organisations').mutationOptions,
  })
}
