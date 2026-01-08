import { useQuery } from '@tanstack/react-query'
import { fetchDeployments } from '../services/deployments-service'

export const useDeployments = () => {
  return useQuery({
    queryKey: ['deployments'],
    queryFn: fetchDeployments,
  })
}
