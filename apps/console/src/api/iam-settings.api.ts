import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

function iamSettingsKey(organisationId: string, deploymentId: string) {
  return window.api.get(
    '/organisations/{organisation_id}/deployments/{deployment_id}/iam-settings',
    { path: { organisation_id: organisationId, deployment_id: deploymentId } }
  ).queryKey
}

export const useGetIamSettings = (organisationId: string | null, deploymentId: string | null) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/deployments/{deployment_id}/iam-settings', {
      path: {
        organisation_id: organisationId ?? 'current',
        deployment_id: deploymentId ?? 'current',
      },
    }).queryOptions,
    enabled: !!organisationId && !!deploymentId && !!accessToken,
  })
}

export const useSetIamSettings = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation(
      'put',
      '/organisations/{organisation_id}/deployments/{deployment_id}/iam-settings'
    ).mutationOptions,
    onSuccess: async (_, variables) => {
      const { organisation_id, deployment_id } = variables.path
      await queryClient.invalidateQueries({
        queryKey: iamSettingsKey(organisation_id, deployment_id),
      })
    },
  })
}
