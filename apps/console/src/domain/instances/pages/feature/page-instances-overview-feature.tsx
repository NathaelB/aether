import { useInstances } from '../../hooks/use-instances';
import { InstancesDashboard } from '../ui/instances-dashboard';
import InstancesLoadingSkeleton from '../ui/instances-loading-skeleton';


export default function InstancesOverviewFeature() {
  const { data: instances, isLoading, error, refetch } = useInstances();

  if (isLoading) return <InstancesLoadingSkeleton />;

  if (error) {
    return (
      <div className="flex items-center justify-center h-100">
        <div className="text-center space-y-2">
          <p className="text-lg font-semibold text-red-600">Error loading instances</p>
          <p className="text-sm text-muted-foreground">
            {error instanceof Error ? error.message : 'An unexpected error occurred'}
          </p>
        </div>
      </div>
    );
  }

  return (
    <InstancesDashboard
      instances={instances || []}
      onRefresh={() => refetch()}
    />
  )
}
