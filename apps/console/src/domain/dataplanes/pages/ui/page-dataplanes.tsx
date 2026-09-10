import type { Schemas } from '@/api/api.client'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Link } from '@tanstack/react-router'
import { formatDistanceToNow } from 'date-fns'
import { formatCpu, formatMemory } from '@/domain/deployments/types/resources'
import {
  DataPlaneAllocationBadge,
  DataPlaneLivenessBadge,
  DataPlaneStatusBadge,
} from './components/dataplane-badges'

interface Props {
  dataplanes: Schemas.DataPlane[]
  isLoading: boolean
  detailPath: (dataplaneId: string) => string
}

export function PageDataPlanes({ dataplanes, isLoading, detailPath }: Props) {
  return (
    <div className='space-y-6'>
      <header className='space-y-1'>
        <h1 className='text-2xl font-semibold tracking-tight'>Data planes</h1>
        <p className='text-sm text-muted-foreground'>
          The Kubernetes clusters deployments run on. Each one pulls its own work — the control
          plane never connects to them.
        </p>
      </header>

      {isLoading ? (
        <Skeleton className='h-40 w-full' />
      ) : dataplanes.length === 0 ? (
        <div className='rounded-lg border border-dashed p-10 text-center'>
          <p className='text-sm font-medium'>No data plane is registered.</p>
          <p className='mt-1 text-sm text-muted-foreground'>
            Nothing can be deployed until one exists. A dedicated deployment provisions its own;
            a shared one needs one to be registered first.
          </p>
        </div>
      ) : (
        <div className='overflow-x-auto rounded-lg border'>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Data plane</TableHead>
                <TableHead>Region</TableHead>
                <TableHead>Allocation</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Heartbeat</TableHead>
                <TableHead className='text-right'>Capacity</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {dataplanes.map((dataplane) => (
                <TableRow key={dataplane.id}>
                  <TableCell>
                    <Link
                      to={detailPath(dataplane.id)}
                      className='font-mono text-xs font-medium hover:underline'
                    >
                      {dataplane.id}
                    </Link>
                  </TableCell>
                  <TableCell>{dataplane.region}</TableCell>
                  <TableCell>
                    <DataPlaneAllocationBadge allocation={dataplane.allocation} />
                  </TableCell>
                  <TableCell>
                    <DataPlaneStatusBadge status={dataplane.status} />
                  </TableCell>
                  <TableCell>
                    <div className='flex flex-col gap-1'>
                      <DataPlaneLivenessBadge dataplane={dataplane} />
                      {dataplane.last_seen_at && (
                        <span className='text-xs text-muted-foreground'>
                          {formatDistanceToNow(new Date(dataplane.last_seen_at))} ago
                        </span>
                      )}
                    </div>
                  </TableCell>
                  <TableCell className='text-right font-mono text-xs text-muted-foreground'>
                    {formatCpu(dataplane.capacity.cpu_millis)} ·{' '}
                    {formatMemory(dataplane.capacity.memory_mib)} ·{' '}
                    {dataplane.capacity.storage_gib} GiB
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  )
}
