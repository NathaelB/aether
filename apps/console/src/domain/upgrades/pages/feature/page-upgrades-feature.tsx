import { useParams } from '@tanstack/react-router'
import { useGetDeployment } from '@/api/deployment.api'
import {
  useGetReleaseAvailability,
  useGetUpgradeInFlight,
  useSetUpgradeSettings,
  useUpgradeDeployment,
} from '@/api/upgrade.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { progressOf } from '../../progress'
import { PageUpgrades } from '../ui/page-upgrades'

export default function PageUpgradesFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()

  const deployment = useGetDeployment(deploymentId ?? null)
  const releases = useGetReleaseAvailability(organisationId ?? null, deploymentId ?? null)
  const inFlight = useGetUpgradeInFlight(organisationId ?? null, deploymentId ?? null)

  const upgrade = useUpgradeDeployment()
  const settings = useSetUpgradeSettings()

  const path = organisationId &&
    deploymentId && { organisation_id: organisationId, deployment_id: deploymentId }

  return (
    <PageUpgrades
      deployment={deployment.data?.data}
      releases={releases.data?.data ?? []}
      progress={progressOf(inFlight.data?.data)}
      isLoading={deployment.isLoading}
      isUpgrading={upgrade.isPending}
      onUpgrade={(version) => {
        if (!path || upgrade.isPending) return
        upgrade.mutate({ path, body: { version } })
      }}
      isSaving={settings.isPending}
      onSaveSettings={(body) => {
        if (!path || settings.isPending) return
        settings.mutate({ path, body })
      }}
    />
  )
}
