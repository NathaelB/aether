import { useState } from 'react'
import { useParams } from '@tanstack/react-router'
import { useGetDeployment } from '@/api/deployment.api'
import { useGetActiveUsers, useGetDeploymentUsage } from '@/api/metrics.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { periodFor, rememberPeriod, rememberedPeriod, type PeriodKey } from '../../periods'
import { aggregationFor, toSeries, type MetricKey } from '../../series'
import { PageUsage, type MetricSeries } from '../ui/page-usage'

const CHARTED: MetricKey[] = ['requests', 'logins', 'token_events', 'active_users']

export default function PageUsageFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()
  const [periodKey, setPeriodKey] = useState<PeriodKey>(() => rememberedPeriod().key)

  const period = periodFor(periodKey)
  const deployment = useGetDeployment(deploymentId ?? null)

  // Pinned when the period is chosen rather than read on every render, so a
  // refetch does not shift every bucket by a few seconds and redraw the whole
  // chart under the reader.
  const [window, setWindow] = useState(() => windowFor(period.minutes))

  const requests = useUsage(organisationId, deploymentId, 'requests', window)
  const logins = useUsage(organisationId, deploymentId, 'logins', window)
  const tokenEvents = useUsage(organisationId, deploymentId, 'token_events', window)
  const activeUsersSeries = useUsage(organisationId, deploymentId, 'active_users', window)

  const activeUsers = useGetActiveUsers(
    organisationId ?? null,
    deploymentId ?? null,
    period.minutes,
  )

  const series: MetricSeries[] = CHARTED.map((metric) => {
    const query = { requests, logins, token_events: tokenEvents, active_users: activeUsersSeries }[
      metric
    ]

    return {
      metric,
      isLoading: query.isLoading,
      points: toSeries(
        query.data?.data ?? [],
        window.from,
        window.until,
        period.slots,
        aggregationFor(metric),
      ),
    }
  })

  return (
    <PageUsage
      deployment={deployment.data?.data}
      period={period}
      onPeriodChange={(key) => {
        setPeriodKey(key)
        setWindow(windowFor(periodFor(key).minutes))
        rememberPeriod(key)
      }}
      series={series}
      activeUsers={activeUsers.data?.data.active_users}
      isLoading={deployment.isLoading}
    />
  )
}

function windowFor(minutes: number): { from: number; until: number } {
  const until = Date.now()
  return { from: until - minutes * 60_000, until }
}

function useUsage(
  organisationId: string | null | undefined,
  deploymentId: string | undefined,
  metric: MetricKey,
  window: { from: number; until: number },
) {
  return useGetDeploymentUsage(
    organisationId ?? null,
    deploymentId ?? null,
    metric,
    new Date(window.from).toISOString(),
    new Date(window.until).toISOString(),
  )
}
