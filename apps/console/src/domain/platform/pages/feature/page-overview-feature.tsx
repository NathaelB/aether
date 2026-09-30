import { useGetDataplanes } from '@/api/dataplane.api'
import { useGetEstateDeployments, useGetTenants } from '@/api/platform.api'
import { useGetOpenSignals } from '@/api/signals.api'
import { WHOLE_ESTATE } from '../../estate-filters'
import { PageOverview } from '../ui/page-overview'

export default function PageOverviewFeature() {
  const signals = useGetOpenSignals()
  const deployments = useGetEstateDeployments(WHOLE_ESTATE)
  const dataplanes = useGetDataplanes()
  const organisations = useGetTenants()

  return (
    <PageOverview
      signals={signals.data?.data ?? []}
      signalsLoading={signals.isLoading}
      deploymentsCount={deployments.data?.data.length}
      deploymentsLoading={deployments.isLoading}
      dataplanesCount={dataplanes.data?.data.length}
      dataplanesLoading={dataplanes.isLoading}
      organisationsCount={organisations.data?.data.length}
      organisationsLoading={organisations.isLoading}
    />
  )
}
