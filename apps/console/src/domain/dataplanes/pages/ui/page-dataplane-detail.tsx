import type { Schemas } from '@/api/api.client'
import { Card, EmptyState, InfoRow, Page, PageTitle, Section } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { HeraldCredential } from './components/herald-credential'
import { Meter } from '@/components/ui/meter'
import { Skeleton } from '@/components/ui/skeleton'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Link } from '@tanstack/react-router'
import { formatDistanceToNow } from 'date-fns'
import { Boxes, CalendarClock, Cpu, HardDrive, MemoryStick, Radio, Server } from 'lucide-react'
import { organisationPathFor } from '@/lib/paths'
import { KIND_LABELS } from '@/domain/deployments/types/deployment'
import { formatCpu, formatMemory, formatStorage } from '@/domain/deployments/types/resources'
import { DeploymentStatusBadge } from '@/domain/deployments/pages/ui/components/deployment-status'
import { allocationOwner, capacityUsage } from '../../capacity'
import {
  DataPlaneAllocationBadge,
  DataPlaneLivenessBadge,
  DataPlaneStatusBadge,
} from './components/dataplane-badges'

type Service = Schemas.ServiceIntent

interface Props {
  dataplane?: Schemas.DataPlane
  deployments: Schemas.Deployment[]
  isLoading: boolean
  /** Whether this operator holds `operate_fleet`. */
  canOperate: boolean
  onSetService: (service: Service) => void
  pending?: Service
  refusal?: string
  onReissue: () => void
  isReissuing: boolean
  /** Present once a new credential has been issued, and only then. */
  reissued?: { clientId: string; clientSecret: string }
  onCredentialDismissed: () => void
  apiUrl: string
  issuerUrl: string
}

/**
 * What each choice does, said rather than named.
 *
 * "Draining" means nothing to somebody deciding whether it is safe to click.
 * What they need to know is that nothing running stops.
 */
const SERVICE_COPY: Record<Service, { label: string; says: string }> = {
  draining: {
    label: 'Drain',
    says: 'No new deployments are placed here. Everything already running keeps running, so this is how a machine is emptied before it is given up.',
  },
  disabled: {
    label: 'Disable',
    says: 'Out of service. The same effect on placement as draining, for a cluster that should not be used right now rather than one on its way out.',
  },
  in_service: {
    label: 'Return to service',
    says: 'Back on the path it was on: available again once its Herald reports.',
  },
}

export function PageDataPlaneDetail({
  dataplane,
  deployments,
  isLoading,
  canOperate,
  onSetService,
  pending,
  refusal,
  onReissue,
  isReissuing,
  reissued,
  onCredentialDismissed,
  apiUrl,
  issuerUrl,
}: Props) {

  if (isLoading || !dataplane) {
    return (
      <Page>
        <Skeleton className='h-10 w-72' />
        <Skeleton className='mt-6 h-48 w-full' />
      </Page>
    )
  }

  const usage = capacityUsage(dataplane.capacity, deployments)
  const owner = allocationOwner(dataplane.allocation)
  const live = deployments.filter((deployment) => !deployment.deleted_at)

  return (
    <Page>
      <PageTitle
        icon={
          <span className='flex h-9 w-9 items-center justify-center rounded-lg border bg-muted/40'>
            <Server className='h-4 w-4 text-muted-foreground' />
          </span>
        }
        title={dataplane.region}
        badges={
          <>
            <DataPlaneStatusBadge status={dataplane.status} />
            <DataPlaneLivenessBadge dataplane={dataplane} />
            <DataPlaneAllocationBadge allocation={dataplane.allocation} />
          </>
        }
      />

      {canOperate && dataplane.status !== 'failed' && (
        <div className='mt-6 flex flex-wrap items-center gap-2 rounded-lg border bg-muted/20 px-4 py-3'>
          <p className='mr-auto text-sm text-muted-foreground'>
            {SERVICE_COPY[
              dataplane.status === 'draining' || dataplane.status === 'disabled'
                ? 'in_service'
                : 'draining'
            ].says}
          </p>

          {dataplane.status === 'draining' || dataplane.status === 'disabled' ? (
            <Button size='sm' disabled={!!pending} onClick={() => onSetService('in_service')}>
              {pending === 'in_service' ? 'Returning…' : SERVICE_COPY.in_service.label}
            </Button>
          ) : (
            <Button
              variant='outline'
              size='sm'
              disabled={!!pending}
              onClick={() => onSetService('draining')}
            >
              {pending === 'draining' ? 'Draining…' : SERVICE_COPY.draining.label}
            </Button>
          )}

          {dataplane.status !== 'disabled' && (
            <Button
              variant='outline'
              size='sm'
              disabled={!!pending}
              onClick={() => onSetService('disabled')}
            >
              {pending === 'disabled' ? 'Disabling…' : SERVICE_COPY.disabled.label}
            </Button>
          )}

          <Button variant='outline' size='sm' disabled={isReissuing} onClick={onReissue}>
            {isReissuing ? 'Issuing…' : 'Re-issue credential'}
          </Button>
        </div>
      )}

      <Sheet open={!!reissued} onOpenChange={(next) => !next && onCredentialDismissed()}>
        <SheetContent className='w-full overflow-y-auto sm:max-w-xl'>
          <SheetHeader>
            <SheetTitle>A new credential</SheetTitle>
            <SheetDescription>
              The previous secret stopped working the moment this was issued. The Herald running
              in this cluster cannot speak until its chart carries the one below.
            </SheetDescription>
          </SheetHeader>

          <div className='px-4 pb-6'>
            {reissued && (
              <HeraldCredential
                dataplaneId={dataplane.id}
                clientId={reissued.clientId}
                clientSecret={reissued.clientSecret}
                apiUrl={apiUrl}
                issuerUrl={issuerUrl}
                onDone={onCredentialDismissed}
              />
            )}
          </div>
        </SheetContent>
      </Sheet>

      {refusal && (
        <p className='mt-4 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive'>
          {refusal}
        </p>
      )}

      <div className='mt-8 space-y-8'>
        <Section
          title='Capacity'
          aside={<span className='text-xs text-muted-foreground'>Reserved</span>}
        >
          <div className='grid gap-4 lg:grid-cols-3'>
            <Card className='flex flex-col justify-between'>
              <div>
                <p className='text-sm text-muted-foreground'>Deployments</p>
                <p className='mt-1 text-4xl font-semibold tabular-nums'>
                  {usage.deploymentCount ? (
                    <>
                      {usage.deploymentCount.used}
                      <span className='text-lg text-muted-foreground'>
                        {' '}
                        / {usage.deploymentCount.total}
                      </span>
                    </>
                  ) : (
                    live.length
                  )}
                </p>
              </div>
              {usage.deploymentCount ? (
                <Meter percent={usage.deploymentCount.percent} className='mt-6 h-2' />
              ) : (
                <p className='mt-6 text-xs text-muted-foreground'>No deployment limit set</p>
              )}
            </Card>

            <Card className='space-y-3 lg:col-span-2'>
              <InfoRow
                icon={<Cpu className='h-4 w-4' />}
                label='CPU reserved'
                value={
                  <>
                    {formatCpu(usage.cpuMillis.used)}
                    <span className='text-muted-foreground'>
                      {' '}
                      / {formatCpu(usage.cpuMillis.total)} · {usage.cpuMillis.percent}%
                    </span>
                  </>
                }
              />
              <InfoRow
                icon={<MemoryStick className='h-4 w-4' />}
                label='Memory reserved'
                value={
                  <>
                    {formatMemory(usage.memoryMib.used)}
                    <span className='text-muted-foreground'>
                      {' '}
                      / {formatMemory(usage.memoryMib.total)} · {usage.memoryMib.percent}%
                    </span>
                  </>
                }
              />
              <InfoRow
                icon={<HardDrive className='h-4 w-4' />}
                label='Storage reserved'
                value={
                  <>
                    {formatStorage(usage.storageGib.used)}
                    <span className='text-muted-foreground'>
                      {' '}
                      / {formatStorage(usage.storageGib.total)} · {usage.storageGib.percent}%
                    </span>
                  </>
                }
              />
              <div className='border-t pt-3'>
                <InfoRow
                  icon={<Radio className='h-4 w-4' />}
                  label='Last reported'
                  value={
                    dataplane.last_seen_at
                      ? `${formatDistanceToNow(new Date(dataplane.last_seen_at))} ago`
                      : 'never'
                  }
                />
              </div>
              {owner && (
                <InfoRow
                  icon={<CalendarClock className='h-4 w-4' />}
                  label='Owner'
                  value={<span className='font-mono text-xs'>{owner}</span>}
                />
              )}
            </Card>
          </div>
        </Section>

        <Section title='Deployments'>
          {live.length === 0 ? (
            <EmptyState
              icon={<Boxes className='h-5 w-5' />}
              title='Nothing placed here yet'
            />
          ) : (
            <div className='overflow-x-auto rounded-lg border'>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Provider</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>CPU</TableHead>
                    <TableHead>Memory</TableHead>
                    <TableHead className='text-right'>Storage</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {live.map((deployment) => (
                    <TableRow key={deployment.id}>
                      <TableCell className='font-medium'>
                        <Link
                          // Through the organisation that owns it: a data
                          // plane hosts several, so the one in the URL cannot
                          // come from wherever the reader happens to be.
                          to={organisationPathFor(
                            deployment.organisation_id,
                            `/deployments/${deployment.id}`,
                          )}
                          className='hover:underline'
                        >
                          {deployment.name}
                        </Link>
                      </TableCell>
                      <TableCell className='text-muted-foreground'>
                        {KIND_LABELS[deployment.kind]}
                      </TableCell>
                      <TableCell>
                        <DeploymentStatusBadge status={deployment.status} />
                      </TableCell>
                      <TableCell className='min-w-32'>
                        <div className='flex items-center gap-2'>
                          <span className='w-14 shrink-0 font-mono text-xs'>
                            {formatCpu(deployment.resources.cpu_millis)}
                          </span>
                          <Meter
                            percent={
                              (deployment.resources.cpu_millis / dataplane.capacity.cpu_millis) * 100
                            }
                          />
                        </div>
                      </TableCell>
                      <TableCell className='min-w-32'>
                        <div className='flex items-center gap-2'>
                          <span className='w-14 shrink-0 font-mono text-xs'>
                            {formatMemory(deployment.resources.memory_mib)}
                          </span>
                          <Meter
                            percent={
                              (deployment.resources.memory_mib / dataplane.capacity.memory_mib) * 100
                            }
                          />
                        </div>
                      </TableCell>
                      <TableCell className='text-right font-mono text-xs text-muted-foreground'>
                        {formatStorage(deployment.resources.storage_gib)}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
        </Section>
      </div>
    </Page>
  )
}
