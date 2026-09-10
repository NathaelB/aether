import type { Schemas } from '@/api/api.client'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { EmptyState, Page, PageTitle, Section } from '@/components/layout/page'
import { formatDistanceToNow } from 'date-fns'
import { Trash2 } from 'lucide-react'
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

/** Namespaces are built as `{environment}-{name}`. */
function environmentOf(namespace: string): string {
  const environment = namespace.split('-')[0]
  return environment.charAt(0).toUpperCase() + environment.slice(1)
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
      <PageTitle
        title={deployment.name}
        badges={
          <>
            <DeploymentStatusBadge status={deployment.status} />
            <span className='text-xs text-muted-foreground'>
              {KIND_LABELS[deployment.kind]} · {deployment.version}
            </span>
          </>
        }
        actions={
          <Button variant='outline' size='sm' onClick={onDelete} disabled={deleting}>
            <Trash2 className='h-4 w-4' />
            Delete
          </Button>
        }
      />

      <div className='mt-8 space-y-8'>
      <Section title='Details'>
        <dl className='grid gap-4 rounded-lg border p-4 sm:grid-cols-3'>
          <Field label='Environment'>{environmentOf(deployment.namespace)}</Field>
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
          <div className='overflow-x-auto rounded-lg border'>
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
          </div>
        )}
      </Section>
      </div>
    </Page>
  )
}
