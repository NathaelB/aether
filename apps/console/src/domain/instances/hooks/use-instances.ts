import { useQuery } from '@tanstack/react-query';
import { fetchInstances } from '../services/instances-service';

export const useInstances = () => {
  return useQuery({
    queryKey: ['instances'],
    queryFn: fetchInstances,
  });
};
