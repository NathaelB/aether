import { useQuery } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

export const useGetDeploymentUsage = (
  organisationId: string | null,
  deploymentId: string | null,
  metric: string,
  from: string,
  until: string,
) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get(
      '/organisations/{organisation_id}/deployments/{deployment_id}/usage-metrics/{metric}',
      {
        path: {
          organisation_id: organisationId ?? 'current',
          deployment_id: deploymentId ?? 'current',
          metric,
        },
        query: { from, until },
      },
    ).queryOptions,
    enabled: !!organisationId && !!deploymentId && !!accessToken,
  })
}

/**
 * How many distinct users a deployment has seen in the last `windowMinutes`.
 *
 * Read from the API rather than derived from the series the chart draws: the
 * platform defines an active user in one place, and a second definition here
 * would quietly disagree with it.
 */
export const useGetActiveUsers = (
  organisationId: string | null,
  deploymentId: string | null,
  windowMinutes: number,
) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get(
      '/organisations/{organisation_id}/deployments/{deployment_id}/active-users',
      {
        path: {
          organisation_id: organisationId ?? 'current',
          deployment_id: deploymentId ?? 'current',
        },
        query: { window_minutes: windowMinutes },
      },
    ).queryOptions,
    enabled: !!organisationId && !!deploymentId && !!accessToken,
  })
}
