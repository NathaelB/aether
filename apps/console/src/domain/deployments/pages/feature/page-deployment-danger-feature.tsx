import { useNavigate, useParams } from '@tanstack/react-router'
import { useDeleteDeployment, useGetDeployment } from '@/api/deployment.api'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { PageDeploymentDanger } from '../ui/page-deployment-danger'

export default function PageDeploymentDangerFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()
  const organisationPath = useOrganisationPath()
  const navigate = useNavigate()

  const deployment = useGetDeployment(deploymentId ?? null)
  const deleteDeployment = useDeleteDeployment()

  return (
    <PageDeploymentDanger
      deployment={deployment.data?.data}
      isLoading={deployment.isLoading}
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
