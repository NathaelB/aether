import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

function networkAccessKey(organisationId: string, deploymentId: string) {
  return window.api.get(
    '/organisations/{organisation_id}/deployments/{deployment_id}/network-access',
    { path: { organisation_id: organisationId, deployment_id: deploymentId } },
  ).queryKey
}

function deploymentKey(organisationId: string, deploymentId: string) {
  return window.api.get('/organisations/{organisation_id}/deployments/{deployment_id}', {
    path: { organisation_id: organisationId, deployment_id: deploymentId },
  }).queryKey
}

/** Which source addresses may reach a deployment, as the platform has it. */
export const useGetNetworkAccess = (
  organisationId: string | null,
  deploymentId: string | null,
) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get(
      '/organisations/{organisation_id}/deployments/{deployment_id}/network-access',
      {
        path: {
          organisation_id: organisationId ?? 'current',
          deployment_id: deploymentId ?? 'current',
        },
      },
    ).queryOptions,
    enabled: !!organisationId && !!deploymentId && !!accessToken,
  })
}

export const useSetNetworkAccess = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation(
      'put',
      '/organisations/{organisation_id}/deployments/{deployment_id}/network-access',
    ).mutationOptions,
    onSuccess: async (_, variables) => {
      const { organisation_id, deployment_id } = variables.path

      // The rule and the deployment both change: the deployment carries its
      // own copy, and a screen reading one while the other is stale shows two
      // answers to the same question.
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: networkAccessKey(organisation_id, deployment_id),
        }),
        queryClient.invalidateQueries({ queryKey: deploymentKey(organisation_id, deployment_id) }),
      ])
    },
  })
}
