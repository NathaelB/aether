import { useParams } from '@tanstack/react-router'
import { useGetDeployment } from '@/api/deployment.api'
import { useGetReleaseAvailability, useGetUpgradeInFlight, useUpgradeDeployment } from '@/api/upgrade.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { progressOf } from '../../progress'
import { PageVersion } from '../ui/page-version'

export default function PageVersionFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()

  const deployment = useGetDeployment(deploymentId ?? null)
  const releases = useGetReleaseAvailability(organisationId ?? null, deploymentId ?? null)
  const inFlight = useGetUpgradeInFlight(organisationId ?? null, deploymentId ?? null)
  const upgrade = useUpgradeDeployment()

  return (
    <PageVersion
      deployment={deployment.data?.data}
      releases={releases.data?.data ?? []}
      progress={progressOf(inFlight.data?.data)}
      isLoading={deployment.isLoading}
      isUpgrading={upgrade.isPending}
      onUpgrade={(version) => {
        if (!organisationId || !deploymentId || upgrade.isPending) return

        upgrade.mutate({
          path: { organisation_id: organisationId, deployment_id: deploymentId },
          body: { version },
        })
      }}
    />
  )
}
