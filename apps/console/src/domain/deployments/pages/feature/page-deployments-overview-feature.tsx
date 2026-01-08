import { useDeployments } from '../../hooks/use-deployments'
import DeploymentsLoadingSkeleton from '../ui/deployments-loading-skeleton'
import { PageDeploymentsOverview } from '../ui/page-deployments-overview'

export default function DeploymentsOverviewFeature() {
  const { data: deployments, isLoading, error, refetch } = useDeployments()

  if (isLoading) return <DeploymentsLoadingSkeleton />

  if (error) {
    return (
      <div className='flex h-[50vh] w-full items-center justify-center'>
        <div className='text-center'>
          <p className='text-lg font-semibold text-red-600'>Error loading deployments</p>
          <p className='text-muted-foreground'>
            {error instanceof Error ? error.message : 'An unexpected error occurred'}
          </p>
        </div>
      </div>
    )
  }

  return (
    <PageDeploymentsOverview
      deployments={deployments || []}
      onRefresh={() => refetch()}
    />
  )
}