import { useDeleteDeployment, useGetDeployments } from '@/api/deployment.api'
import DeploymentsLoadingSkeleton from '../ui/deployments-loading-skeleton'
import { PageDeploymentsOverview } from '../ui/page-deployments-overview'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'

export default function DeploymentsOverviewFeature() {
  const { data, isLoading, error, refetch } = useGetDeployments()
  const deleteDeployment = useDeleteDeployment()
  const organisationId = useResolvedOrganisationId()
  const deployments = data?.data ?? []

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
      deployments={deployments}
      organisationId={organisationId}
      onDelete={(deploymentId) => {
        if (!organisationId || deleteDeployment.isPending) {
          return
        }

        deleteDeployment.mutate({
          path: {
            organisation_id: organisationId,
            deployment_id: deploymentId,
          },
        })
      }}
      onRefresh={() => refetch()}
    />
  )
}
