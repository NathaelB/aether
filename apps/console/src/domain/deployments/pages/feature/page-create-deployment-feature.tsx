import { useNavigate } from '@tanstack/react-router'
import PageCreateDeployment from '../ui/page-create-deployment'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { useCreateDeployment } from '@/api/deployment.api'
import { useGetRegions } from '@/api/region.api'
import { useGetPublishedReleases } from '@/api/release.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import {
  toCreateDeploymentRequest,
  type CreateDeploymentForm,
} from '../../create-deployment-request'

export default function PageCreateDeploymentFeature() {
  const navigate = useNavigate()
  const organisationPath = useOrganisationPath()
  const organisationId = useResolvedOrganisationId()
  const createDeployment = useCreateDeployment()
  const regions = useGetRegions()

  // Both products, because the form lets the choice change and a version list
  // that arrives after the click is a list nobody saw.
  const ferriskey = useGetPublishedReleases('ferriskey')
  const keycloak = useGetPublishedReleases('keycloak')

  const handleCreate = (form: CreateDeploymentForm) => {
    if (!organisationId) return

    createDeployment.mutate(
      {
        path: { organisation_id: organisationId },
        body: toCreateDeploymentRequest(form),
      },
      { onSuccess: () => navigate({ to: organisationPath('/deployments') }) },
    )
  }

  return (
    <PageCreateDeployment
      onSubmit={handleCreate}
      isSubmitting={createDeployment.isPending}
      regions={regions.data?.data ?? []}
      regionsLoading={regions.isLoading}
      releases={{
        ferriskey: ferriskey.data?.data ?? [],
        keycloak: keycloak.data?.data ?? [],
      }}
      releasesLoading={ferriskey.isLoading || keycloak.isLoading}
    />
  )
}
