import type { Schemas } from '@/api/api.client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Skeleton } from '@/components/ui/skeleton'
import { formatDistanceToNow } from 'date-fns'
import { Boxes, Cpu, HardDrive, MemoryStick } from 'lucide-react'
// Resource formatting belongs to the resource concept, which the deployments
// domain already owns and tests. A second copy here would be a second place
// for `2048` to start rendering as something other than `2 GiB`.
import {
  formatCpu,
  formatMemory,
  formatStorage,
} from '@/domain/deployments/types/resources'
import { capacityUsage, allocationOwner } from '../../capacity'
import { CapacityBar } from './components/capacity-bar'
import {
  DataPlaneAllocationBadge,
  DataPlaneLivenessBadge,
  DataPlaneStatusBadge,
} from './components/dataplane-badges'

interface Props {
  dataplane?: Schemas.DataPlane
  deployments: Schemas.Deployment[]
  isLoading: boolean
}

export function PageDataPlaneDetail({ dataplane, deployments, isLoading }: Props) {
  if (isLoading || !dataplane) {
    return (
      <div className='space-y-6'>
        <Skeleton className='h-9 w-72' />
        <Skeleton className='h-48 w-full' />
      </div>
    )
  }

  const usage = capacityUsage(dataplane.capacity, deployments)
  const owner = allocationOwner(dataplane.allocation)
  const live = deployments.filter((deployment) => !deployment.deleted_at)

  return (
    <div className='space-y-8'>
      <header className='space-y-3'>
        <div className='flex items-center gap-3'>
          <Boxes className='h-6 w-6 text-muted-foreground' aria-hidden='true' />
          <h1 className='font-mono text-2xl font-semibold tracking-tight'>{dataplane.id}</h1>
        </div>
        <div className='flex flex-wrap items-center gap-2'>
          <DataPlaneStatusBadge status={dataplane.status} />
          <DataPlaneLivenessBadge dataplane={dataplane} />
          <DataPlaneAllocationBadge allocation={dataplane.allocation} />
          <span className='inline-flex items-center rounded-md border bg-background px-2 py-0.5 text-xs font-medium text-muted-foreground'>
            {dataplane.region}
          </span>
        </div>
      </header>

      <section className='space-y-4'>
        <h2 className='text-sm font-semibold text-muted-foreground'>Data plane information</h2>

        <div className='grid gap-4 lg:grid-cols-3'>
          <Card>
            <CardHeader className='pb-3'>
              <CardTitle className='text-sm font-medium text-muted-foreground'>
                Deployments
              </CardTitle>
            </CardHeader>
            <CardContent>
              <p className='text-4xl font-semibold tabular-nums'>{live.length}</p>
              <p className='mt-1 text-xs text-muted-foreground'>placed on this data plane</p>
            </CardContent>
          </Card>

          <Card className='lg:col-span-2'>
            <CardHeader className='pb-3'>
              <CardTitle className='text-sm font-medium text-muted-foreground'>
                Reserved capacity
              </CardTitle>
            </CardHeader>
            <CardContent className='space-y-4'>
              {/*
                Reserved, not measured. These are the numbers placement
                subtracted when it chose this data plane -- a deployment idling
                at 3% CPU still holds every millicore it asked for, because that
                is what stopped something else being placed on top of it.
              */}
              <CapacityBar label='CPU' usage={usage.cpuMillis} format={formatCpu} />
              <CapacityBar label='Memory' usage={usage.memoryMib} format={formatMemory} />
              <CapacityBar label='Storage' usage={usage.storageGib} format={formatStorage} />
            </CardContent>
          </Card>
        </div>

        <div className='grid gap-4 sm:grid-cols-3'>
          <InfoRow icon={Cpu} label='Last reported'>
            {dataplane.last_seen_at
              ? `${formatDistanceToNow(new Date(dataplane.last_seen_at))} ago`
              : 'never'}
          </InfoRow>
          <InfoRow icon={MemoryStick} label='Allocation'>
            {owner ? <span className='font-mono text-xs'>{owner}</span> : 'shared'}
          </InfoRow>
          <InfoRow icon={HardDrive} label='Region'>
            {dataplane.region}
          </InfoRow>
        </div>
      </section>

      <section className='space-y-4'>
        <h2 className='text-sm font-semibold text-muted-foreground'>Deployments</h2>

        {live.length === 0 ? (
          <p className='rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground'>
            Nothing is placed on this data plane yet.
          </p>
        ) : (
          <div className='overflow-x-auto rounded-lg border'>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Namespace</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className='text-right'>CPU</TableHead>
                  <TableHead className='text-right'>Memory</TableHead>
                  <TableHead className='text-right'>Storage</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {live.map((deployment) => (
                  <TableRow key={deployment.id}>
                    <TableCell className='font-medium'>{deployment.name}</TableCell>
                    <TableCell className='font-mono text-xs text-muted-foreground'>
                      {deployment.namespace}
                    </TableCell>
                    <TableCell>{deployment.status}</TableCell>
                    <TableCell className='text-right font-mono text-xs'>
                      {formatCpu(deployment.resources.cpu_millis)}
                    </TableCell>
                    <TableCell className='text-right font-mono text-xs'>
                      {formatMemory(deployment.resources.memory_mib)}
                    </TableCell>
                    <TableCell className='text-right font-mono text-xs'>
                      {formatStorage(deployment.resources.storage_gib)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </section>
    </div>
  )
}

function InfoRow({
  icon: Icon,
  label,
  children,
}: {
  icon: typeof Cpu
  label: string
  children: React.ReactNode
}) {
  return (
    <div className='flex items-center gap-3 rounded-lg border p-3'>
      <Icon className='h-4 w-4 shrink-0 text-muted-foreground' aria-hidden='true' />
      <div className='min-w-0'>
        <p className='text-xs text-muted-foreground'>{label}</p>
        <p className='truncate text-sm font-medium'>{children}</p>
      </div>
    </div>
  )
}
