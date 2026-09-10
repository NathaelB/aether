import { useParams } from '@tanstack/react-router'
import { useGetDataplane, useGetDataplaneDeployments } from '@/api/dataplane.api'
import { PageDataPlaneDetail } from '../ui/page-dataplane-detail'

export default function PageDataPlaneDetailFeature() {
  const { dataplaneId } = useParams({ strict: false }) as { dataplaneId?: string }
  const dataplane = useGetDataplane(dataplaneId ?? null)
  const deployments = useGetDataplaneDeployments(dataplaneId ?? null)

  return (
    <PageDataPlaneDetail
      dataplane={dataplane.data?.data}
      deployments={deployments.data?.data ?? []}
      isLoading={dataplane.isLoading}
    />
  )
}
