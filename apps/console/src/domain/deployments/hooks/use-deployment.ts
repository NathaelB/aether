import { useQuery } from '@tanstack/react-query'
import { getDeploymentById } from '../services/deployments-service'

export const useDeployment = (id: string) => {
  return useQuery({
    queryKey: ['deployment', id],
    queryFn: () => getDeploymentById(id),
    enabled: !!id,
  })
}
