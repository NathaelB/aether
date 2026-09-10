import { useGetDataplanes } from '@/api/dataplane.api'
import { PageDataPlanes } from '../ui/page-dataplanes'

export default function PageDataPlanesFeature() {
  const dataplanes = useGetDataplanes()

  return (
    <PageDataPlanes dataplanes={dataplanes.data?.data ?? []} isLoading={dataplanes.isLoading} />
  )
}
