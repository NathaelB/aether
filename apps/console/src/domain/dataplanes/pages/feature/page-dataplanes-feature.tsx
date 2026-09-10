import { useGetDataplanes } from '@/api/dataplane.api'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { PageDataPlanes } from '../ui/page-dataplanes'

export default function PageDataPlanesFeature() {
  const dataplanes = useGetDataplanes()
  const organisationPath = useOrganisationPath()

  return (
    <PageDataPlanes
      dataplanes={dataplanes.data?.data ?? []}
      isLoading={dataplanes.isLoading}
      detailPath={(dataplaneId) => organisationPath(`/dataplanes/${dataplaneId}`)}
    />
  )
}
