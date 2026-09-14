import type { Schemas } from '@/api/api.client'
import { EmptyState, Page, PageTitle } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { DeploymentStatusBadge } from '@/domain/deployments/pages/ui/components/deployment-status'
import { Link } from '@tanstack/react-router'
import { format } from 'date-fns'
import { ArrowLeft, Boxes, Building2, DatabaseBackup } from 'lucide-react'
import { platformPath } from '@/lib/paths'

interface Props {
  tenant?: Schemas.Tenant
  deployments: Schemas.EstateDeployment[]
  isLoading: boolean
  asking?: string
  onAskForBackup: (deployment: Schemas.EstateDeployment) => void
  outcome?: { ok: boolean; message: string }
}

/** One fact about an organisation, in the place every other one is drawn. */
function Fact({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className='rounded-lg border bg-card p-4'>
      <p className='text-xs uppercase tracking-wide text-muted-foreground'>{label}</p>
      <p className='mt-1 truncate text-lg font-semibold'>{value}</p>
    </div>
  )
}

export function PageTenantDetail({
  tenant,
  deployments,
  isLoading,
  asking,
  onAskForBackup,
  outcome,
}: Props) {
  if (isLoading) {
    return (
      <Page>
        <Skeleton className='h-9 w-64' />
        <div className='mt-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4'>
          <Skeleton className='h-24' />
          <Skeleton className='h-24' />
          <Skeleton className='h-24' />
          <Skeleton className='h-24' />
        </div>
      </Page>
    )
  }

  if (!tenant) {
    return (
      <Page>
        <EmptyState
          icon={<Building2 className='h-5 w-5' />}
          title='No such organisation'
          description='It may have been deleted since this link was made.'
        />
      </Page>
    )
  }

  const { organisation } = tenant

  return (
    <Page>
      <Link
        to={platformPath('/organisations')}
        className='mb-4 inline-flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground'
      >
        <ArrowLeft className='h-3.5 w-3.5' />
        Organisations
      </Link>

      <PageTitle
        icon={<Building2 className='h-5 w-5 text-muted-foreground' />}
        title={organisation.name}
      />

      <div className='mt-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4'>
        <Fact label='Plan' value={organisation.plan} />
        <Fact label='Status' value={organisation.status} />
        <Fact
          label='Deployments'
          // Against the limit, not alone: a tenant at four of five is about to
          // have a conversation with somebody, and the number on its own does
          // not say that.
          value={
            <span className='tabular-nums'>
              {tenant.deployments}
              <span className='text-muted-foreground'>
                {' / '}
                {organisation.limits.max_instances}
              </span>
            </span>
          }
        />
        <Fact
          label='Members'
          value={
            <span className='tabular-nums'>
              {tenant.members}
              <span className='text-muted-foreground'>
                {' / '}
                {organisation.limits.max_users}
              </span>
            </span>
          }
        />
      </div>

      <dl className='mt-6 grid gap-x-8 gap-y-3 border-t pt-6 text-sm sm:grid-cols-2'>
        <div className='flex justify-between gap-4'>
          <dt className='text-muted-foreground'>Slug</dt>
          <dd className='font-mono'>{organisation.slug}</dd>
        </div>
        <div className='flex justify-between gap-4'>
          <dt className='text-muted-foreground'>Storage allowance</dt>
          <dd className='tabular-nums'>{organisation.limits.max_storage_gb} GiB</dd>
        </div>
        <div className='flex justify-between gap-4'>
          <dt className='text-muted-foreground'>Created</dt>
          <dd>{format(new Date(organisation.created_at), 'd MMM yyyy')}</dd>
        </div>
        <div className='flex justify-between gap-4'>
          <dt className='text-muted-foreground'>Owner</dt>
          <dd className='truncate font-mono text-xs'>{organisation.owner_id}</dd>
        </div>
      </dl>

      {outcome && (
        <div
          className={
            outcome.ok
              ? 'mt-6 rounded-lg border border-primary/30 bg-primary/5 px-4 py-3 text-sm'
              : 'mt-6 rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm'
          }
          role='status'
        >
          {outcome.message}
        </div>
      )}

      {/* Below the organisation's own facts rather than instead of them: an
          operator opening a tenant wants to know what it is before what it
          runs, and then wants what it runs. */}
      <h2 className='mt-10 text-lg font-semibold'>Deployments</h2>

      <div className='mt-4'>
        {deployments.length === 0 ? (
          <EmptyState
            icon={<Boxes className='h-5 w-5' />}
            title='Nothing deployed'
            description='This organisation has not deployed anything on this installation.'
          />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Deployment</TableHead>
                <TableHead>Environment</TableHead>
                <TableHead>Offer</TableHead>
                <TableHead>Region</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className='text-right'>Archive</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {deployments.map((row) => (
                <TableRow key={row.deployment.id}>
                  <TableCell>
                    <p className='font-medium'>{row.deployment.name}</p>
                    <p className='font-mono text-xs text-muted-foreground'>
                      {row.deployment.kind} {row.deployment.version}
                    </p>
                  </TableCell>
                  <TableCell className='text-muted-foreground'>
                    {row.deployment.environment}
                  </TableCell>
                  <TableCell className='text-muted-foreground'>
                    {row.deployment.offer ?? '—'}
                  </TableCell>
                  <TableCell className='font-mono text-xs'>{row.region}</TableCell>
                  <TableCell>
                    <DeploymentStatusBadge status={row.deployment.status} />
                  </TableCell>
                  <TableCell className='text-right'>
                    <Button
                      variant='outline'
                      size='sm'
                      disabled={asking === row.deployment.id}
                      onClick={() => onAskForBackup(row)}
                    >
                      <DatabaseBackup className='mr-1.5 h-3.5 w-3.5' />
                      {asking === row.deployment.id ? 'Asking…' : 'Back up now'}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </div>
    </Page>
  )
}
