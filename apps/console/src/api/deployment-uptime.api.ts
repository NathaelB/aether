import { useQuery } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

export const useGetDeploymentUptime = (deploymentId: string | null, enabled: boolean) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/platform/deployments/{deployment_id}/uptime', {
      path: { deployment_id: deploymentId ?? 'current' },
    }).queryOptions,
    enabled: enabled && !!deploymentId && !!accessToken,
  })
}
