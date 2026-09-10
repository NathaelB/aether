import { useGetDeployments } from '@/api/deployment.api'
import { useGetDataplanes } from '@/api/dataplane.api'
import { PageDashboard } from '../ui/page-dashboard'

export default function PageDashboardFeature() {
  const deployments = useGetDeployments()
  const dataplanes = useGetDataplanes()

  return (
    <PageDashboard
      deployments={deployments.data?.data ?? []}
      dataplanes={dataplanes.data?.data ?? []}
      isLoading={deployments.isLoading || dataplanes.isLoading}
    />
  )
}
