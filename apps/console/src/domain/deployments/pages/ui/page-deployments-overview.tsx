import { Button } from '@/components/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import {
  PlusCircle,
  Server,
  MoreVertical,
  ExternalLink,
  Copy,
  Trash2,
  RefreshCw,
  Search,
} from 'lucide-react'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import type { Deployment, DeploymentKind } from '../../types/deployment'
import { useState } from 'react'
import { Link } from '@tanstack/react-router'
import * as Dialog from '@radix-ui/react-dialog'
import { DeploymentStatusBadge } from './components/deployment-status'

interface Props {
  deployments: Deployment[];
  organisationId: string | null;
  onDelete: (deploymentId: string) => void;
  onRefresh: () => void;
}

const kindConfig: Record<DeploymentKind, { label: string; color: string }> = {
  keycloak: { label: 'Keycloak', color: 'text-blue-700 bg-blue-100' },
  ferriskey: { label: 'Ferriskey', color: 'text-purple-700 bg-purple-100' },
}

export const PageDeploymentsOverview = ({
  deployments,
  organisationId,
  onDelete,
  onRefresh,
}: Props) => {
  const [searchQuery, setSearchQuery] = useState('')
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; name?: string } | null>(null)

  const filteredDeployments = deployments.filter(deployment =>
    deployment.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    deployment.id.toLowerCase().includes(searchQuery.toLowerCase())
  )

  const stats = {
    total: deployments.length,
    successful: deployments.filter(i => i.status === 'successful').length,
    failed: deployments.filter(i => i.status === 'failed').length,
    in_progress: deployments.filter(i => i.status === 'in_progress').length,
  }

  return (
    <div className='space-y-6'>
      {/* Header */}
      <div className='flex items-center justify-between'>
        <div>
          <h2 className='text-2xl font-bold tracking-tight'>Deployments</h2>
          <p className='text-sm text-muted-foreground'>
            Manage and monitor your IAM deployments
          </p>
        </div>
        {organisationId && (
          <Button className='gap-2' asChild>
            <Link
              to='/organisations/$organisationId/deployments/create'
              params={{ organisationId }}
            >
              <PlusCircle className='h-4 w-4' />
              New Deployment
            </Link>
          </Button>
        )}
      </div>

      {/* Stats Cards */}
      <div className='grid gap-4 md:grid-cols-4'>
        <Card>
          <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
            <CardTitle className='text-sm font-medium'>Total Deployments</CardTitle>
            <Server className='h-4 w-4 text-muted-foreground' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{stats.total}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
            <CardTitle className='text-sm font-medium'>Successful</CardTitle>
            <div className='h-2 w-2 rounded-full bg-green-500' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{stats.successful}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
            <CardTitle className='text-sm font-medium'>In Progress</CardTitle>
            <div className='h-2 w-2 rounded-full bg-blue-500' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{stats.in_progress}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
            <CardTitle className='text-sm font-medium'>Failed</CardTitle>
            <div className='h-2 w-2 rounded-full bg-red-500' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{stats.failed}</div>
          </CardContent>
        </Card>
      </div>

      {/* Search and Filters */}
      <div className='flex items-center gap-2'>
        <div className='relative flex-1'>
          <Search className='absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground' />
          <Input
            placeholder='Search deployments by name or ID...'
            className='pl-8'
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>
        <Button variant='outline' size='icon' onClick={onRefresh}>
          <RefreshCw className='h-4 w-4' />
        </Button>
      </div>

      {/* Deployments Table */}
      <Card>
        <CardContent className='p-0'>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Kind</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Version</TableHead>
                <TableHead>Namespace</TableHead>
                <TableHead>Deployed</TableHead>
                <TableHead className='text-right'>Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filteredDeployments.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={7} className='text-center h-24 text-muted-foreground'>
                    No deployments found
                  </TableCell>
                </TableRow>
              ) : (
                filteredDeployments.map((deployment) => {
                  const isDeleting = !!deployment.deleted_at
                  const kindInfo = kindConfig[deployment.kind] ?? {
                    label: deployment.kind,
                    color: 'text-gray-600 bg-gray-50',
                  }
                  const deploymentLabel = (
                    <>
                      <span className='font-medium'>{deployment.name}</span>
                      <span className='text-xs text-muted-foreground font-mono'>
                        {deployment.id}
                      </span>
                    </>
                  )

                  return (
                    <TableRow key={deployment.id}>
                      <TableCell>
                        {organisationId ? (
                          <Link
                            to='/organisations/$organisationId/deployments/$deploymentId'
                            params={{ organisationId, deploymentId: deployment.id }}
                            className='flex flex-col hover:underline'
                          >
                            {deploymentLabel}
                          </Link>
                        ) : (
                          <div className='flex flex-col'>{deploymentLabel}</div>
                        )}
                      </TableCell>
                      <TableCell>
                        <span className={`inline-flex items-center rounded-md px-2 py-1 text-xs font-medium ${kindInfo.color}`}>
                          {kindInfo.label}
                        </span>
                      </TableCell>
                      <TableCell>
                        <DeploymentStatusBadge status={deployment.status} />
                      </TableCell>
                      <TableCell className='text-sm'>{deployment.version}</TableCell>
                      <TableCell className='text-sm font-mono text-muted-foreground'>{deployment.namespace}</TableCell>
                      <TableCell className='text-sm'>
                        {deployment.deployed_at
                          ? new Date(deployment.deployed_at).toLocaleDateString()
                          : '—'}
                      </TableCell>
                      <TableCell className='text-right'>
                        <div className='flex items-center justify-end gap-2'>
                          {isDeleting ? (
                            <span className='text-xs text-muted-foreground'>Deletion in progress</span>
                          ) : (
                            <DropdownMenu>
                              <DropdownMenuTrigger asChild>
                                <Button variant='ghost' size='icon' className='h-8 w-8'>
                                  <MoreVertical className='h-4 w-4' />
                                </Button>
                              </DropdownMenuTrigger>
                              <DropdownMenuContent align='end'>
                                <DropdownMenuItem>
                                  <ExternalLink className='mr-2 h-4 w-4' />
                                  Open Console
                                </DropdownMenuItem>
                                <DropdownMenuItem>
                                  <Copy className='mr-2 h-4 w-4' />
                                  Copy ID
                                </DropdownMenuItem>
                                <DropdownMenuItem>View Logs</DropdownMenuItem>
                                <DropdownMenuSeparator />
                                <DropdownMenuItem
                                  className='text-red-600'
                                  onSelect={() =>
                                    setDeleteTarget({
                                      id: deployment.id,
                                      name: deployment.name,
                                    })
                                  }
                                >
                                  <Trash2 className='mr-2 h-4 w-4' />
                                  Delete
                                </DropdownMenuItem>
                              </DropdownMenuContent>
                            </DropdownMenu>
                          )}
                        </div>
                      </TableCell>
                    </TableRow>
                  )
                })
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
      <Dialog.Root
        open={!!deleteTarget}
        onOpenChange={(open) => {
          if (!open) {
            setDeleteTarget(null)
          }
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className='fixed inset-0 bg-black/40 backdrop-blur-sm' />
          <Dialog.Content className='fixed left-1/2 top-1/2 w-[90vw] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg border bg-background p-6 shadow-xl'>
            <Dialog.Title className='text-lg font-semibold'>Delete deployment</Dialog.Title>
            <Dialog.Description className='mt-2 text-sm text-muted-foreground'>
              This will permanently delete
              {deleteTarget?.name ? ` "${deleteTarget.name}"` : ' this deployment'}.
              This action cannot be undone.
            </Dialog.Description>
            <div className='mt-6 flex justify-end gap-2'>
              <Dialog.Close asChild>
                <Button variant='outline'>Cancel</Button>
              </Dialog.Close>
              <Button
                variant='destructive'
                onClick={() => {
                  if (!deleteTarget) return
                  onDelete(deleteTarget.id)
                  setDeleteTarget(null)
                }}
              >
                Delete
              </Button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </div>
  )
}
