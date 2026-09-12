import type { Schemas } from '@/api/api.client'
import { EmptyState, Page, PageTitle, Section } from '@/components/layout/page'
import { Skeleton } from '@/components/ui/skeleton'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { TrendingDown, TrendingUp } from 'lucide-react'
import { PERIODS, type Period, type PeriodKey } from '../../periods'
import {
  METRIC_LABELS,
  aggregationFor,
  formatCount,
  growth,
  type MetricKey,
  type Point,
} from '../../series'
import { UsageChart } from './usage-chart'

export interface MetricSeries {
  metric: MetricKey
  points: Point[]
  isLoading: boolean
}

interface Props {
  deployment?: Schemas.Deployment
  period: Period
  onPeriodChange: (key: PeriodKey) => void
  series: MetricSeries[]
  activeUsers: number | null | undefined
  isLoading: boolean
}

/**
 * Minutes within a day, days beyond one. A timestamp to the second on a week
 * long axis is noise, and a date on an hour long one says nothing.
 */
function timeFormatter(period: Period): (at: number) => string {
  const withinADay = period.minutes <= 60 * 24

  return (at) =>
    new Date(at).toLocaleString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
      ...(withinADay ? {} : { day: '2-digit', month: 'short' }),
    })
}

function Growth({ points, metric }: { points: Point[]; metric: MetricKey }) {
  const change = growth(points, aggregationFor(metric))

  if (change === null) {
    return <span className='text-xs text-muted-foreground'>No comparison</span>
  }

  const up = change >= 0
  const Icon = up ? TrendingUp : TrendingDown

  return (
    <span
      className={`inline-flex items-center gap-1 text-xs tabular-nums ${
        up ? 'text-green-600 dark:text-green-400' : 'text-amber-600 dark:text-amber-400'
      }`}
    >
      <Icon className='h-3.5 w-3.5' />
      {up ? '+' : ''}
      {Math.round(change * 100)}%
    </span>
  )
}

export function PageUsage({
  deployment,
  period,
  onPeriodChange,
  series,
  activeUsers,
  isLoading,
}: Props) {
  if (isLoading || !deployment) {
    return (
      <Page>
        <Skeleton className='h-9 w-64' />
        <Skeleton className='mt-8 h-40 w-full' />
      </Page>
    )
  }

  const formatTime = timeFormatter(period)

  return (
    <Page>
      <PageTitle
        title='Usage'
        badges={<span className='text-xs text-muted-foreground'>{deployment.name}</span>}
        actions={
          <Tabs value={period.key} onValueChange={(value) => onPeriodChange(value as PeriodKey)}>
            <TabsList>
              {PERIODS.map(({ key, label }) => (
                <TabsTrigger key={key} value={key}>
                  {label}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
        }
      />

      <div className='mt-8 space-y-8'>
        <Section title='Active users'>
          <div className='rounded-lg border p-5'>
            {activeUsers === null || activeUsers === undefined ? (
              <p className='text-sm text-muted-foreground'>
                Nothing was reported in this period, so there is no number to show. This is not
                the same as nobody being active.
              </p>
            ) : (
              <div className='flex items-baseline gap-3'>
                <span className='text-3xl font-semibold tabular-nums'>
                  {formatCount(activeUsers)}
                </span>
                <span className='text-sm text-muted-foreground'>
                  distinct users in the {period.label.toLowerCase().replace('last ', '')}
                </span>
              </div>
            )}
          </div>
        </Section>

        {series.map(({ metric, points, isLoading: loading }) => (
          <Section
            key={metric}
            title={METRIC_LABELS[metric]}
            aside={loading ? null : <Growth points={points} metric={metric} />}
          >
            {loading ? (
              <Skeleton className='h-40 w-full' />
            ) : points.length === 0 ? (
              <EmptyState title='Nothing reported' />
            ) : (
              <UsageChart points={points} formatTime={formatTime} />
            )}
          </Section>
        ))}
      </div>
    </Page>
  )
}
