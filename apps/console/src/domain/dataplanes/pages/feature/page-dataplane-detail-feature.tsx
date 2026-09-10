import { useParams } from '@tanstack/react-router'
import { useGetDataplane, useGetDataplaneDeployments } from '@/api/dataplane.api'
import { PageDataPlaneDetail } from '../ui/page-dataplane-detail'

export default function PageDataPlaneDetailFeature() {
  const { dataplaneId } = useParams({ strict: false }) as { dataplaneId?: string }
  const dataplane = useGetDataplane(dataplaneId ?? null)
  const deployments = useGetDataplaneDeployments(dataplaneId ?? null)

  return (
    <PageDataPlaneDetail
      // No `.data` here, and that is not a slip: `GET /dataplanes/{id}` is
      // the one endpoint that returns the object bare rather than wrapped in
      // an envelope like every other one. Filed rather than papered over.
      dataplane={dataplane.data}
      deployments={deployments.data?.data ?? []}
      isLoading={dataplane.isLoading}
    />
  )
}
