import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import type { Schemas } from '@/api/api.client'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

/**
 * What a customer may install: everything the catalogue holds except what has
 * only been planned. Takes no deployment, because a deployment being created
 * does not exist yet.
 */
export const useGetPublishedReleases = (kind: Schemas.DeploymentKind) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/releases/{kind}', { path: { kind } }).queryOptions,
    enabled: !!accessToken,
  })
}

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

/** Adds a version to the catalogue. Nothing can be installed until one is here. */
export const usePublishRelease = (kind: Schemas.DeploymentKind) => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation('post', '/releases/operator/{kind}').mutationOptions,
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: window.api.get('/releases/operator/{kind}', { path: { kind } }).queryKey,
      })
    },
  })
}

/**
 * Widens who a release is offered to. Only ever widens: a version somebody
 * has already taken cannot be un-offered, which is what withdrawing is for.
 */
export const useWidenRollout = (kind: Schemas.DeploymentKind) => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation('put', '/releases/operator/{kind}/{version}/rollout').mutationOptions,
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: window.api.get('/releases/operator/{kind}', { path: { kind } }).queryKey,
      })
    },
  })
}
