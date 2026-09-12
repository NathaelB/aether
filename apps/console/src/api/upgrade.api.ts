import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

/**
 * Every release of this product as this deployment sees it, with a reason
 * attached to each one it may not install. Not the plain catalogue listing: a
 * release still being rolled out exists without being on offer here.
 */
export const useGetReleaseAvailability = (
  organisationId: string | null,
  deploymentId: string | null,
) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/releases/deployments/{organisation_id}/{deployment_id}', {
      path: {
        organisation_id: organisationId ?? 'current',
        deployment_id: deploymentId ?? 'current',
      },
    }).queryOptions,
    enabled: !!organisationId && !!deploymentId && !!accessToken,
  })
}

/** The upgrade being applied right now, with the path it is following. */
export const useGetUpgradeInFlight = (
  organisationId: string | null,
  deploymentId: string | null,
) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/deployments/{deployment_id}/upgrade', {
      path: {
        organisation_id: organisationId ?? 'current',
        deployment_id: deploymentId ?? 'current',
      },
    }).queryOptions,
    enabled: !!organisationId && !!deploymentId && !!accessToken,
    // An upgrade moves through its path on its own, so the screen follows it
    // rather than waiting for someone to reload.
    refetchInterval: 10_000,
  })
}

function deploymentKey(organisationId: string, deploymentId: string) {
  return window.api.get('/organisations/{organisation_id}/deployments/{deployment_id}', {
    path: { organisation_id: organisationId, deployment_id: deploymentId },
  }).queryKey
}

function inFlightKey(organisationId: string, deploymentId: string) {
  return window.api.get('/organisations/{organisation_id}/deployments/{deployment_id}/upgrade', {
    path: { organisation_id: organisationId, deployment_id: deploymentId },
  }).queryKey
}

export const useSetUpgradeSettings = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation(
      'put',
      '/organisations/{organisation_id}/deployments/{deployment_id}/upgrade-settings',
    ).mutationOptions,
    onSuccess: async (_, variables) => {
      await queryClient.invalidateQueries({
        queryKey: deploymentKey(variables.path.organisation_id, variables.path.deployment_id),
      })
    },
  })
}

export const useUpgradeDeployment = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation(
      'post',
      '/organisations/{organisation_id}/deployments/{deployment_id}/upgrade',
    ).mutationOptions,
    onSuccess: async (_, variables) => {
      const { organisation_id, deployment_id } = variables.path

      // The upgrade that was just accepted, as well as the deployment. Without
      // this the screen waits for the next poll before showing anything, so a
      // click reads as nothing having happened for up to ten seconds.
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: deploymentKey(organisation_id, deployment_id) }),
        queryClient.invalidateQueries({ queryKey: inFlightKey(organisation_id, deployment_id) }),
      ])
    },
  })
}
