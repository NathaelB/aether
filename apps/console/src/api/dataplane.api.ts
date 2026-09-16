import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

export const useGetDataplanes = () => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/dataplanes').queryOptions,
    enabled: !!accessToken,
  })
}

export const useGetDataplane = (dataplaneId: string | null) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/dataplanes/{dataplane_id}', {
      path: { dataplane_id: dataplaneId ?? 'current' },
    }).queryOptions,
    enabled: !!dataplaneId && !!accessToken,
  })
}

/**
 * What is placed on a data plane.
 *
 * The endpoint shards for Herald's benefit -- each Herald claims its own slice
 * -- so the console asks for the whole set explicitly rather than inheriting
 * a default that was chosen for a different caller.
 */
export const useGetDataplaneDeployments = (dataplaneId: string | null) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/dataplanes/{dataplane_id}/deployments', {
      path: { dataplane_id: dataplaneId ?? 'current' },
      query: { shard_index: 0, shard_count: 1 },
    }).queryOptions,
    enabled: !!dataplaneId && !!accessToken,
  })
}

/**
 * Taking a data plane out of service, or putting it back.
 *
 * Both the plane and its deployments are invalidated: draining changes what
 * the cluster will accept without touching what is on it, and a screen that
 * refetched only one of the two would show a drained plane still described as
 * a placement candidate.
 */
export const useSetDataplaneService = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation('put', '/dataplanes/{dataplane_id}/service').mutationOptions,
    onSuccess: async (_, variables) => {
      await queryClient.invalidateQueries({
        queryKey: window.api.get('/dataplanes/{dataplane_id}', {
          path: { dataplane_id: variables.path.dataplane_id },
        }).queryKey,
      })
      await queryClient.invalidateQueries({
        queryKey: window.api.get('/dataplanes').queryKey,
      })
    },
  })
}
