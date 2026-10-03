import { useParams } from '@tanstack/react-router'
import { useGetIamFeatures } from '@/api/iam-features.api'
import { useGetIamSettings, useSetIamSettings } from '@/api/iam-settings.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { toApplyRequest, toResetRequest } from '../../branding'
import { PageBranding } from '../ui/page-branding'

export default function PageBrandingFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()

  const features = useGetIamFeatures(organisationId ?? null)
  const settings = useGetIamSettings(organisationId ?? null, deploymentId ?? null)
  const save = useSetIamSettings()

  const open = features.data?.data.features.find((entry) => entry.feature === 'branding')

  const send = (body: Parameters<typeof save.mutate>[0]['body']) => {
    if (!organisationId || !deploymentId || save.isPending) return

    save.mutate({
      path: { organisation_id: organisationId, deployment_id: deploymentId },
      body,
    })
  }

  return (
    <PageBranding
      isLoading={features.isLoading || (open?.open === true && settings.isLoading)}
      feature={open}
      state={settings.data?.data}
      isSaving={save.isPending}
      error={save.error?.message}
      onApply={(form) => send(toApplyRequest(form))}
      onReset={() => send(toResetRequest())}
    />
  )
}
