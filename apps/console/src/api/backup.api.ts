import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

function scheduleKey(organisationId: string, deploymentId: string) {
  return window.api.get(
    '/organisations/{organisation_id}/deployments/{deployment_id}/backup-schedule',
    { path: { organisation_id: organisationId, deployment_id: deploymentId } },
  ).queryKey
}

/** The archives a deployment has, as the platform holds them. */
export const useGetBackups = (organisationId: string | null, deploymentId: string | null) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/deployments/{deployment_id}/backups', {
      path: {
        organisation_id: organisationId ?? 'current',
        deployment_id: deploymentId ?? 'current',
      },
    }).queryOptions,
    enabled: !!organisationId && !!deploymentId && !!accessToken,
  })
}

export const useGetBackupSchedule = (
  organisationId: string | null,
  deploymentId: string | null,
) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get(
      '/organisations/{organisation_id}/deployments/{deployment_id}/backup-schedule',
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

export const useSetBackupSchedule = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation(
      'put',
      '/organisations/{organisation_id}/deployments/{deployment_id}/backup-schedule',
    ).mutationOptions,
    onSuccess: async (_, variables) => {
      const { organisation_id, deployment_id } = variables.path

      // Refetched rather than written from the response optimistically. The
      // schedule that applies is the one the platform stored, and a screen
      // drawing its own request is a screen that shows a change nobody made.
      await queryClient.invalidateQueries({
        queryKey: scheduleKey(organisation_id, deployment_id),
      })
    },
  })
}
