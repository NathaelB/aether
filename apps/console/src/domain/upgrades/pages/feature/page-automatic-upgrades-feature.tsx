import { useParams } from '@tanstack/react-router'
import { useGetDeployment } from '@/api/deployment.api'
import { useSetUpgradeSettings } from '@/api/upgrade.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { PageAutomaticUpgrades } from '../ui/page-automatic-upgrades'

export default function PageAutomaticUpgradesFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()

  const deployment = useGetDeployment(deploymentId ?? null)
  const settings = useSetUpgradeSettings()

  return (
    <PageAutomaticUpgrades
      deployment={deployment.data?.data}
      isLoading={deployment.isLoading}
      isSaving={settings.isPending}
      onSave={(body) => {
        if (!organisationId || !deploymentId || settings.isPending) return

        settings.mutate({
          path: { organisation_id: organisationId, deployment_id: deploymentId },
          body,
        })
      }}
    />
  )
}
