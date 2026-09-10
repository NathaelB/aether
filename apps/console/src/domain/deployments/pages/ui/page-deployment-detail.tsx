import type { Schemas } from '@/api/api.client'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { DataTable, EmptyState, Page, PageHeader, Section } from '@/components/layout/page'
import { Link } from '@tanstack/react-router'
import { formatDistanceToNow } from 'date-fns'
import { Trash2 } from 'lucide-react'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { KIND_LABELS } from '../../types/deployment'
import { formatCpu, formatMemory, formatStorage } from '../../types/resources'
import { DeploymentStatusBadge } from './components/deployment-status'

interface Props {
  deployment?: Schemas.Deployment
  actions: Schemas.Action[]
  isLoading: boolean
  onDelete: () => void
}

function actionStatusLabel(status: Schemas.ActionStatus): string {
  return typeof status === 'string' ? status : Object.keys(status)[0]
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className='space-y-1'>
      <dt className='text-xs text-muted-foreground'>{label}</dt>
      <dd className='text-sm'>{children}</dd>
    </div>
  )
}

export function PageDeploymentDetail({ deployment, actions, isLoading, onDelete }: Props) {
  const organisationPath = useOrganisationPath()

  if (isLoading || !deployment) {
    return (
      <Page>
        <Skeleton className='h-9 w-64' />
        <Skeleton className='h-40 w-full' />
      </Page>
    )
  }

  const deleting = deployment.status === 'deleting' || deployment.status === 'deleted'

  return (
    <Page>
      <PageHeader
        title={
          <span className='flex items-center gap-3'>
            {deployment.name}
            <DeploymentStatusBadge status={deployment.status} />
          </span>
        }
        description={`${KIND_LABELS[deployment.kind]} · ${deployment.version}`}
        actions={
          <Button variant='outline' size='sm' onClick={onDelete} disabled={deleting}>
            <Trash2 className='h-4 w-4' />
            Delete
          </Button>
        }
      />

      <Section title='Details'>
        <dl className='grid gap-4 rounded-lg border p-4 sm:grid-cols-3'>
          <Field label='Namespace'>
            <span className='font-mono text-xs'>{deployment.namespace}</span>
          </Field>
          <Field label='Data plane'>
            <Link
              to={organisationPath(`/dataplanes/${deployment.dataplane_id}`)}
              className='font-mono text-xs hover:underline'
            >
              {deployment.dataplane_id}
            </Link>
          </Field>
          <Field label='Created'>
            {formatDistanceToNow(new Date(deployment.created_at))} ago
          </Field>
          <Field label='CPU'>{formatCpu(deployment.resources.cpu_millis)}</Field>
          <Field label='Memory'>{formatMemory(deployment.resources.memory_mib)}</Field>
          <Field label='Storage'>{formatStorage(deployment.resources.storage_gib)}</Field>
        </dl>
      </Section>

      <Section title='Activity'>
        {actions.length === 0 ? (
          <EmptyState title='Nothing recorded yet' />
        ) : (
          <DataTable>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Action</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className='text-right'>When</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {actions.map((action) => (
                  <TableRow key={action.id}>
                    <TableCell className='font-mono text-xs'>{action.action_type}</TableCell>
                    <TableCell className='text-muted-foreground'>
                      {actionStatusLabel(action.status)}
                    </TableCell>
                    <TableCell className='text-right text-xs text-muted-foreground'>
                      {formatDistanceToNow(new Date(action.metadata.created_at))} ago
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </DataTable>
        )}
      </Section>
    </Page>
  )
}
