import { format, formatDistanceToNow } from 'date-fns'
import { AlertTriangle, Boxes, Building2, LayoutDashboard, Server } from 'lucide-react'
import { Link } from '@tanstack/react-router'
import type { Schemas } from '@/api/api.client'
import { Card, EmptyState, Page, PageTitle, Section } from '@/components/layout/page'
import { Skeleton } from '@/components/ui/skeleton'
import { StatusBadge } from '@/components/ui/status-badge'
import { platformPath } from '@/lib/paths'
import { signalKindLabel, signalSubjectLabel } from '../../signal-labels'

interface StatProps {
  label: string
  value: number | undefined
  isLoading: boolean
  icon: React.ReactNode
  to: string
}

function Stat({ label, value, isLoading, icon, to }: StatProps) {
  return (
    <Link to={to}>
      <Card className='flex items-center gap-4 transition-colors hover:bg-accent/50'>
        <div className='rounded-md bg-muted p-2.5 text-muted-foreground'>{icon}</div>
        <div>
          {isLoading ? (
            <Skeleton className='h-8 w-12' />
          ) : (
            <div className='text-2xl font-semibold tabular-nums'>{value ?? '—'}</div>
          )}
          <div className='text-sm text-muted-foreground'>{label}</div>
        </div>
      </Card>
    </Link>
  )
}

function SignalRow({ signal }: { signal: Schemas.Signal }) {
  return (
    <li className='flex flex-wrap items-baseline gap-x-3 gap-y-1 p-4'>
      <StatusBadge tone='warning'>{signalKindLabel(signal.kind)}</StatusBadge>
      <span className='font-mono text-xs text-muted-foreground'>
        {signalSubjectLabel(signal.subject)}
      </span>
      <span className='text-sm'>{signal.message}</span>
      <time
        className='ml-auto text-xs text-muted-foreground'
        dateTime={signal.opened_at}
        title={format(new Date(signal.opened_at), 'PPpp')}
      >
        {formatDistanceToNow(new Date(signal.opened_at), { addSuffix: true })}
      </time>
    </li>
  )
}

interface Props {
  signals: Schemas.Signal[]
  signalsLoading: boolean
  deploymentsCount: number | undefined
  deploymentsLoading: boolean
  dataplanesCount: number | undefined
  dataplanesLoading: boolean
  organisationsCount: number | undefined
  organisationsLoading: boolean
}

export function PageOverview({
  signals,
  signalsLoading,
  deploymentsCount,
  deploymentsLoading,
  dataplanesCount,
  dataplanesLoading,
  organisationsCount,
  organisationsLoading,
}: Props) {
  return (
    <Page>
      <PageTitle icon={<LayoutDashboard className='h-6 w-6' />} title='Overview' />

      <div className='mt-6 grid grid-cols-1 gap-4 sm:grid-cols-3'>
        <Stat
          label='Deployments'
          value={deploymentsCount}
          isLoading={deploymentsLoading}
          icon={<Boxes className='h-5 w-5' />}
          to={platformPath('/deployments')}
        />
        <Stat
          label='Data planes'
          value={dataplanesCount}
          isLoading={dataplanesLoading}
          icon={<Server className='h-5 w-5' />}
          to={platformPath('/dataplanes')}
        />
        <Stat
          label='Organisations'
          value={organisationsCount}
          isLoading={organisationsLoading}
          icon={<Building2 className='h-5 w-5' />}
          to={platformPath('/organisations')}
        />
      </div>

      <div className='mt-8'>
        <Section title='Open signals'>
          {signalsLoading ? (
            <div className='space-y-2'>
              <Skeleton className='h-16' />
              <Skeleton className='h-16' />
            </div>
          ) : signals.length === 0 ? (
            <EmptyState
              icon={<AlertTriangle className='h-5 w-5' />}
              title='Nothing to report'
              description='No probe has found a problem on the fleet right now.'
            />
          ) : (
            <ul className='divide-y rounded-lg border bg-card'>
              {signals.map((signal) => (
                <SignalRow key={signal.id} signal={signal} />
              ))}
            </ul>
          )}
        </Section>
      </div>
    </Page>
  )
}
