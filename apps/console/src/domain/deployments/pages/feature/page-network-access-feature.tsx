import { useParams } from '@tanstack/react-router'
import { useGetDeployment } from '@/api/deployment.api'
import { useGetNetworkAccess, useSetNetworkAccess } from '@/api/network-access.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { PageNetworkAccess } from '../ui/page-network-access'

export default function PageNetworkAccessFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()

  const deployment = useGetDeployment(deploymentId ?? null)
  const access = useGetNetworkAccess(organisationId ?? null, deploymentId ?? null)
  const save = useSetNetworkAccess()

  return (
    <PageNetworkAccess
      deployment={deployment.data?.data}
      access={access.data?.data}
      isLoading={deployment.isLoading || access.isLoading}
      isSaving={save.isPending}
      onApply={(allowed) => {
        if (!organisationId || !deploymentId || save.isPending) return

        save.mutate({
          path: { organisation_id: organisationId, deployment_id: deploymentId },
          body: { allowed_cidrs: allowed },
        })
      }}
    />
  )
}
