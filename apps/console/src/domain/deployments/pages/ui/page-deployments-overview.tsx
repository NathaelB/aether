import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { DataTable, EmptyState, Page, PageHeader } from '@/components/layout/page'
import { Link } from '@tanstack/react-router'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { formatDistanceToNow } from 'date-fns'
import { MoreHorizontal, Plus, RefreshCw, Search, Trash2 } from 'lucide-react'
import { useState } from 'react'
import type { Deployment } from '../../types/deployment'
import { KIND_LABELS } from '../../types/deployment'
import { formatCpu, formatMemory } from '../../types/resources'
import { DeploymentStatusBadge } from './components/deployment-status'

interface Props {
  deployments: Deployment[]
  isLoading: boolean
  onDelete: (deploymentId: string) => void
  onRefresh: () => void
}

export const PageDeploymentsOverview = ({
  deployments,
  isLoading,
  onDelete,
  onRefresh,
}: Props) => {
  const organisationPath = useOrganisationPath()
  const [search, setSearch] = useState('')

  const query = search.trim().toLowerCase()
  const visible = query
    ? deployments.filter(
        (d) => d.name.toLowerCase().includes(query) || d.namespace.toLowerCase().includes(query),
      )
    : deployments

  return (
    <Page>
      <PageHeader
        title='Deployments'
        description='Identity providers running on your data planes.'
        actions={
          <>
            <Button variant='outline' size='sm' onClick={onRefresh} disabled={isLoading}>
              <RefreshCw className='h-4 w-4' />
              Refresh
            </Button>
            <Button size='sm' asChild>
              <Link to={organisationPath('/deployments/create')}>
                <Plus className='h-4 w-4' />
                New deployment
              </Link>
            </Button>
          </>
        }
      />

      {deployments.length > 0 && (
        <div className='relative max-w-sm'>
          <Search className='absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground' />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder='Filter by name or namespace'
            className='pl-8'
          />
        </div>
      )}

      {deployments.length === 0 ? (
        <EmptyState
          title={isLoading ? 'Loading…' : 'No deployments yet'}
          description={
            isLoading ? undefined : 'Create one to run an identity provider on a data plane.'
          }
          action={
            !isLoading && (
              <Button size='sm' asChild>
                <Link to={organisationPath('/deployments/create')}>
                  <Plus className='h-4 w-4' />
                  New deployment
                </Link>
              </Button>
            )
          }
        />
      ) : visible.length === 0 ? (
        <EmptyState title='Nothing matches that filter' />
      ) : (
        <DataTable>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Provider</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Namespace</TableHead>
                <TableHead className='text-right'>Size</TableHead>
                <TableHead className='text-right'>Created</TableHead>
                <TableHead className='w-10' />
              </TableRow>
            </TableHeader>
            <TableBody>
              {visible.map((deployment) => (
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
                  <TableCell className='font-mono text-xs text-muted-foreground'>
                    {deployment.namespace}
                  </TableCell>
                  <TableCell className='text-right font-mono text-xs text-muted-foreground'>
                    {formatCpu(deployment.resources.cpu_millis)} ·{' '}
                    {formatMemory(deployment.resources.memory_mib)}
                  </TableCell>
                  <TableCell className='text-right text-xs text-muted-foreground'>
                    {formatDistanceToNow(new Date(deployment.created_at))} ago
                  </TableCell>
                  <TableCell>
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button variant='ghost' size='icon' className='h-8 w-8'>
                          <MoreHorizontal className='h-4 w-4' />
                          <span className='sr-only'>Actions</span>
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align='end'>
                        <DropdownMenuItem
                          variant='destructive'
                          onClick={() => onDelete(deployment.id)}
                        >
                          <Trash2 className='h-4 w-4' />
                          Delete
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </DataTable>
      )}
    </Page>
  )
}
