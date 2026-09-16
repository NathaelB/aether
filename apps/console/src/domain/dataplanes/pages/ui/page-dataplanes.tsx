import type { Schemas } from '@/api/api.client'
import { EmptyState, Page, PageTitle } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Link } from '@tanstack/react-router'
import { formatDistanceToNow } from 'date-fns'
import { Plus, Server } from 'lucide-react'
import { platformPath } from '@/lib/paths'
import { formatCpu, formatMemory } from '@/domain/deployments/types/resources'
import {
  DataPlaneAllocationBadge,
  DataPlaneLivenessBadge,
  DataPlaneStatusBadge,
} from './components/dataplane-badges'

interface Props {
  dataplanes: Schemas.DataPlane[]
  isLoading: boolean
  /** Whether this operator holds `operate_fleet`. */
  canOperate: boolean
  onRegister: () => void
}

export function PageDataPlanes({ dataplanes, isLoading, canOperate, onRegister }: Props) {

  return (
    <Page>
      <PageTitle
        title='Data planes'
        actions={
          canOperate && (
            <Button size='sm' onClick={onRegister}>
              <Plus className='h-4 w-4' />
              Register a data plane
            </Button>
          )
        }
      />

      <div className='mt-6'>
        {isLoading ? (
          <div className='grid gap-4 sm:grid-cols-2 lg:grid-cols-3'>
            <Skeleton className='h-44' />
            <Skeleton className='h-44' />
          </div>
        ) : dataplanes.length === 0 ? (
          <EmptyState
            icon={<Server className='h-5 w-5' />}
            title='No data plane registered'
            description='Nothing can be deployed until a cluster is registered. Install k3s on a machine you have, register it here, and it announces itself.'
            action={
              canOperate && (
                <Button size='sm' onClick={onRegister}>
                  <Plus className='h-4 w-4' />
                  Register a data plane
                </Button>
              )
            }
          />
        ) : (
          <div className='grid gap-4 sm:grid-cols-2 lg:grid-cols-3'>
            {dataplanes.map((dataplane) => (
              <Link
                key={dataplane.id}
                to={platformPath(`/dataplanes/${dataplane.id}`)}
                className='group rounded-lg border bg-card p-4 transition-colors hover:border-primary/40'
              >
                <div className='flex items-start justify-between gap-3'>
                  <div className='flex min-w-0 items-start gap-3'>
                    <span className='flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border bg-muted/40'>
                      <Server className='h-4 w-4 text-muted-foreground' />
                    </span>
                    <div className='min-w-0'>
                      <p className='truncate font-semibold group-hover:text-primary'>
                        {dataplane.region}
                      </p>
                      <p className='truncate font-mono text-xs text-muted-foreground'>
                        {dataplane.id.slice(0, 8)}
                      </p>
                    </div>
                  </div>
                  <DataPlaneStatusBadge status={dataplane.status} />
                </div>

                <div className='mt-4 flex flex-wrap gap-2'>
                  <DataPlaneAllocationBadge allocation={dataplane.allocation} />
                  <DataPlaneLivenessBadge dataplane={dataplane} />
                </div>

                <div className='mt-4 flex items-center justify-between border-t pt-3 text-xs text-muted-foreground'>
                  <span className='font-mono'>
                    {formatCpu(dataplane.capacity.cpu_millis)} ·{' '}
                    {formatMemory(dataplane.capacity.memory_mib)}
                  </span>
                  <span>
                    {dataplane.last_seen_at
                      ? `${formatDistanceToNow(new Date(dataplane.last_seen_at))} ago`
                      : 'never seen'}
                  </span>
                </div>
              </Link>
            ))}
          </div>
        )}
      </div>
    </Page>
  )
}
