import type { Schemas } from '@/api/api.client'
import { Card, EmptyState, Page, PageTitle, Section } from '@/components/layout/page'
import { Skeleton } from '@/components/ui/skeleton'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { formatDistanceToNow } from 'date-fns'
import { KIND_LABELS, environmentOf } from '../../types/deployment'
import { DeploymentStatusBadge } from './components/deployment-status'

interface Props {
  deployment?: Schemas.Deployment
  actions: Schemas.Action[]
  isLoading: boolean
}

function actionStatusLabel(status: Schemas.ActionStatus): string {
  return typeof status === 'string' ? status : Object.keys(status)[0]
}

function Stat({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <Card className='p-4'>
      <p className='text-sm text-muted-foreground'>{label}</p>
      <p className='mt-1 text-lg font-semibold'>{value}</p>
    </Card>
  )
}

export function PageDeploymentDetail({ deployment, actions, isLoading }: Props) {
  if (isLoading || !deployment) {
    return (
      <Page>
        <Skeleton className='h-9 w-64' />
        <Skeleton className='mt-8 h-40 w-full' />
      </Page>
    )
  }

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
      />

      <div className='mt-8 space-y-8'>
        <div className='grid gap-4 sm:grid-cols-3'>
          <Stat label='Version' value={<span className='font-mono'>{deployment.version}</span>} />
          <Stat label='Environment' value={environmentOf(deployment.namespace)} />
          <Stat
            label='Created'
            value={`${formatDistanceToNow(new Date(deployment.created_at))} ago`}
          />
        </div>

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
