import { useNavigate, useParams } from '@tanstack/react-router'
import {
  useDeleteDeployment,
  useGetDeployment,
  useGetDeploymentActions,
} from '@/api/deployment.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { PageDeploymentDetail } from '../ui/page-deployment-detail'

export default function PageDeploymentDetailFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()
  const organisationPath = useOrganisationPath()
  const navigate = useNavigate()

  const deployment = useGetDeployment(deploymentId ?? null)
  const actions = useGetDeploymentActions(deploymentId ?? null)
  const deleteDeployment = useDeleteDeployment()

  return (
    <PageDeploymentDetail
      deployment={deployment.data?.data}
      actions={actions.data?.data ?? []}
      isLoading={deployment.isLoading}
      onOpenUpgrades={() =>
        navigate({ to: organisationPath(`/deployments/${deploymentId}/upgrades`) })
      }
      onOpenUsage={() => navigate({ to: organisationPath(`/deployments/${deploymentId}/usage`) })}
      onDelete={() => {
        if (!organisationId || !deploymentId || deleteDeployment.isPending) return

        deleteDeployment.mutate(
          { path: { organisation_id: organisationId, deployment_id: deploymentId } },
          { onSuccess: () => navigate({ to: organisationPath('/deployments') }) },
        )
      }}
    />
  )
}
