import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { anySettling, settlingRefetchInterval } from '@/domain/deployments/settling'
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
    refetchInterval: (query) => anySettling(query.state.data?.data),
  })
}

/**
 * One deployment, kept fresh for as long as the platform is still moving it.
 *
 * An upgrade rewrites the version without the console asking, so a record
 * read while one runs is stale -- and a stale version is what makes the page
 * offer an upgrade to the version it has just applied.
 */
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
    refetchInterval: (query) => settlingRefetchInterval(query.state.data?.data?.status),
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

export const useGetDeploymentActions = (deploymentId: string | null) => {
  const organisationId = useResolvedOrganisationId()
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/deployments/{deployment_id}/actions', {
      path: {
        organisation_id: organisationId ?? 'current',
        deployment_id: deploymentId ?? 'current',
      },
      query: { limit: 50 },
    }).queryOptions,
    enabled: !!organisationId && !!deploymentId && !!accessToken,
  })
}
