import { useGetDeployments } from '@/api/deployment.api'
import { useGetDataplanes } from '@/api/dataplane.api'
import {
  selectActiveOrganisationId,
  selectOrganisations,
  useOrganisationsStore,
} from '@/stores/organisations'
import { PageDashboard } from '../ui/page-dashboard'

export default function PageDashboardFeature() {
  const deployments = useGetDeployments()
  const dataplanes = useGetDataplanes()
  const organisations = useOrganisationsStore(selectOrganisations)
  const activeId = useOrganisationsStore(selectActiveOrganisationId)

  const active = organisations.find((organisation) => organisation.id === activeId)

  return (
    <PageDashboard
      organisationName={active?.name ?? 'Overview'}
      deployments={deployments.data?.data ?? []}
      dataplanes={dataplanes.data?.data ?? []}
      isLoading={deployments.isLoading || dataplanes.isLoading}
    />
  )
}
