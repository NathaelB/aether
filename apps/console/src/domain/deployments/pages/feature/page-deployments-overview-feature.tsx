import { useDeleteDeployment, useGetDeployments } from '@/api/deployment.api'
import { PageDeploymentsOverview } from '../ui/page-deployments-overview'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { EmptyState, Page } from '@/components/layout/page'

export default function DeploymentsOverviewFeature() {
  const { data, isLoading, error, refetch } = useGetDeployments()
  const deleteDeployment = useDeleteDeployment()
  const organisationId = useResolvedOrganisationId()

  if (error) {
    return (
      <Page>
        <EmptyState
          title='Could not load deployments'
          description={error instanceof Error ? error.message : 'An unexpected error occurred.'}
        />
      </Page>
    )
  }

  return (
    <PageDeploymentsOverview
      deployments={data?.data ?? []}
      isLoading={isLoading}
      onDelete={(deploymentId) => {
        if (!organisationId || deleteDeployment.isPending) return

        deleteDeployment.mutate({
          path: { organisation_id: organisationId, deployment_id: deploymentId },
        })
      }}
      onRefresh={() => void refetch()}
    />
  )
}
