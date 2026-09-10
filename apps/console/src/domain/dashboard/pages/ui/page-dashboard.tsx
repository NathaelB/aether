import { DataTable, EmptyState, Page, PageHeader, Section } from '@/components/layout/page'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Button } from '@/components/ui/button'
import { Link } from '@tanstack/react-router'
import { formatDistanceToNow } from 'date-fns'
import { Plus } from 'lucide-react'
import type { Schemas } from '@/api/api.client'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { KIND_LABELS } from '@/domain/deployments/types/deployment'
import { DeploymentStatusBadge } from '@/domain/deployments/pages/ui/components/deployment-status'
import { DataPlaneStatusBadge } from '@/domain/dataplanes/pages/ui/components/dataplane-badges'

interface Props {
  deployments: Schemas.Deployment[]
  dataplanes: Schemas.DataPlane[]
  isLoading: boolean
}

function Stat({ label, value }: { label: string; value: number | string }) {
  return (
    <div className='rounded-lg border p-4'>
      <p className='text-sm text-muted-foreground'>{label}</p>
      <p className='mt-1 text-2xl font-semibold tabular-nums'>{value}</p>
    </div>
  )
}

export const PageDashboard = ({ deployments, dataplanes, isLoading }: Props) => {
  const organisationPath = useOrganisationPath()
  const running = deployments.filter((d) => d.status === 'successful').length
  const failed = deployments.filter((d) => d.status === 'failed').length
  const recent = deployments.slice(0, 5)

  return (
    <Page>
      <PageHeader
        title='Overview'
        actions={
          <Button size='sm' asChild>
            <Link to={organisationPath('/deployments/create')}>
              <Plus className='h-4 w-4' />
              New deployment
            </Link>
          </Button>
        }
      />

      <div className='grid gap-4 sm:grid-cols-2 lg:grid-cols-4'>
        <Stat label='Deployments' value={isLoading ? '—' : deployments.length} />
        <Stat label='Running' value={isLoading ? '—' : running} />
        <Stat label='Failed' value={isLoading ? '—' : failed} />
        <Stat label='Data planes' value={isLoading ? '—' : dataplanes.length} />
      </div>

      <Section
        title='Recent deployments'
        actions={
          deployments.length > 0 && (
            <Link
              to={organisationPath('/deployments')}
              className='text-sm text-muted-foreground hover:underline'
            >
              View all
            </Link>
          )
        }
      >
        {recent.length === 0 ? (
          <EmptyState
            title={isLoading ? 'Loading…' : 'No deployments yet'}
            description={isLoading ? undefined : 'Create one to get started.'}
          />
        ) : (
          <DataTable>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Provider</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className='text-right'>Created</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {recent.map((deployment) => (
                  <TableRow key={deployment.id}>
                    <TableCell className='font-medium'>
                      <Link
                        to={organisationPath(`/deployments/${deployment.id}`)}
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
                    <TableCell className='text-right text-xs text-muted-foreground'>
                      {formatDistanceToNow(new Date(deployment.created_at))} ago
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </DataTable>
        )}
      </Section>

      {dataplanes.length > 0 && (
        <Section
          title='Data planes'
          actions={
            <Link
              to={organisationPath('/dataplanes')}
              className='text-sm text-muted-foreground hover:underline'
            >
              View all
            </Link>
          }
        >
          <DataTable>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Data plane</TableHead>
                  <TableHead>Region</TableHead>
                  <TableHead>Status</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {dataplanes.map((dataplane) => (
                  <TableRow key={dataplane.id}>
                    <TableCell>
                      <Link
                        to={organisationPath(`/dataplanes/${dataplane.id}`)}
                        className='font-mono text-xs hover:underline'
                      >
                        {dataplane.id}
                      </Link>
                    </TableCell>
                    <TableCell>{dataplane.region}</TableCell>
                    <TableCell>
                      <DataPlaneStatusBadge status={dataplane.status} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </DataTable>
        </Section>
      )}
    </Page>
  )
}
