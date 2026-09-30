import { useGetDataplanes } from '@/api/dataplane.api'
import { useGetDataplaneUpgrades, useStartDataplaneUpgrade } from '@/api/dataplane-upgrades.api'
import { useHoldsPlatformRight } from '@/domain/organisations/hooks/use-is-operator'
import { toRows } from '../../dataplane-upgrades'
import { PageDataplaneUpgrades } from '../ui/page-dataplane-upgrades'

export default function PageDataplaneUpgradesFeature() {
  const upgrades = useGetDataplaneUpgrades()
  const dataplanes = useGetDataplanes()
  const start = useStartDataplaneUpgrade()
  const canOperate = useHoldsPlatformRight('operate_fleet')

  return (
    <PageDataplaneUpgrades
      rows={toRows(upgrades.data?.data ?? [])}
      dataplanes={dataplanes.data?.data ?? []}
      isLoading={upgrades.isLoading}
      canOperate={canOperate}
      isStarting={start.isPending}
      refusal={start.error instanceof Error ? start.error.message : undefined}
      started={start.isSuccess}
      onStart={(body) => start.mutate({ body })}
    />
  )
}
