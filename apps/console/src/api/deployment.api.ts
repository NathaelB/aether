import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

export const useGetDeployments = () => {
  const organisationId = useResolvedOrganisationId()
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/deployments', {
      path: {
        organisation_id: organisationId ?? 'current',
      },
    }).queryOptions,
    enabled: !!organisationId && !!accessToken,
  })
}

export const useGetDeployment = (deploymentId: string | null) => {
  const organisationId = useResolvedOrganisationId()
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/deployments/{deployment_id}', {
      path: {
        organisation_id: organisationId ?? 'current',
        deployment_id: deploymentId ?? 'current',
      },
    }).queryOptions,
    enabled: !!organisationId && !!deploymentId && !!accessToken,
  })
}

export const useCreateDeployment = () => {
  const queryClient = useQueryClient()
  return useMutation({
    ...window.api.mutation('post', '/organisations/{organisation_id}/deployments').mutationOptions,
    onSuccess: async (_, variables) => {
      const keys = window.api.get('/organisations/{organisation_id}/deployments', {
        path: {
          organisation_id: variables.path.organisation_id,
        },
      }).queryKey

      await queryClient.invalidateQueries({ queryKey: keys })
    },
  })
}

export const useDeleteDeployment = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation('delete', '/organisations/{organisation_id}/deployments/{deployment_id}')
      .mutationOptions,
    onSuccess: async (_, variables) => {
      const keys = window.api.get('/organisations/{organisation_id}/deployments', {
        path: {
          organisation_id: variables.path.organisation_id,
        },
      }).queryKey

      await queryClient.invalidateQueries({ queryKey: keys })
    },
  })
}
