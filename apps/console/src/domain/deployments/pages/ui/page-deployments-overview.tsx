import { Button } from '@/components/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import {
  PlusCircle,
  Server,
  Play,
  Square,
  MoreVertical,
  ExternalLink,
  Copy,
  Trash2,
  RefreshCw,
  Search,
  Users
} from 'lucide-react'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import type { Deployment, DeploymentStatus, DeploymentType } from '../../types/deployment'
import { DEPLOYMENT_PLANS } from '../../types/deployment'
import { useState } from 'react'
import { Link } from '@tanstack/react-router'
import * as Dialog from '@radix-ui/react-dialog'

interface Props {
  deployments: Deployment[];
  organisationId: string | null;
  onDelete: (deploymentId: string) => void;
  onRefresh: () => void;
}

const statusConfig: Record<DeploymentStatus, { label: string; color: string; dotColor: string }> = {
  running: { label: 'Running', color: 'text-green-600 bg-green-50', dotColor: 'bg-green-500' },
  stopped: { label: 'Stopped', color: 'text-gray-600 bg-gray-50', dotColor: 'bg-gray-400' },
  deploying: { label: 'Deploying', color: 'text-blue-600 bg-blue-50', dotColor: 'bg-blue-500' },
  maintenance: { label: 'Maintenance', color: 'text-yellow-600 bg-yellow-50', dotColor: 'bg-yellow-500' },
  error: { label: 'Error', color: 'text-red-600 bg-red-50', dotColor: 'bg-red-500' },
  deleting: { label: 'Deleting', color: 'text-amber-700 bg-amber-50', dotColor: 'bg-amber-500' },
}

const typeConfig: Record<DeploymentType, { label: string; color: string }> = {
  keycloak: { label: 'Keycloak', color: 'text-blue-700 bg-blue-100' },
  ferriskey: { label: 'Ferriskey', color: 'text-purple-700 bg-purple-100' },
  authentik: { label: 'Authentik', color: 'text-orange-700 bg-orange-100' },
}

export const PageDeploymentsOverview = ({
  deployments,
  organisationId,
  onDelete,
  onRefresh,
}: Props) => {
  const [searchQuery, setSearchQuery] = useState('')
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; name?: string } | null>(null)
  const basePath = organisationId ? `/organisations/${organisationId}` : ''

  const filteredDeployments = deployments.filter(deployment =>
    deployment.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    deployment.id.toLowerCase().includes(searchQuery.toLowerCase())
  )

  const stats = {
    total: deployments.length,
    running: deployments.filter(i => i.status === 'running').length,
    stopped: deployments.filter(i => i.status === 'stopped').length,
    error: deployments.filter(i => i.status === 'error').length,
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
        <Button className='gap-2' asChild>
          <Link to={`${basePath}/deployments/create`}>
            <PlusCircle className='h-4 w-4' />
            New Deployment
          </Link>
        </Button>
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
            <CardTitle className='text-sm font-medium'>Running</CardTitle>
            <div className='h-2 w-2 rounded-full bg-green-500' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{stats.running}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
            <CardTitle className='text-sm font-medium'>Stopped</CardTitle>
            <div className='h-2 w-2 rounded-full bg-gray-400' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{stats.stopped}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
            <CardTitle className='text-sm font-medium'>Errors</CardTitle>
            <div className='h-2 w-2 rounded-full bg-red-500' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{stats.error}</div>
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
                <TableHead>Type</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Version</TableHead>
                <TableHead>Region</TableHead>
                <TableHead>Plan</TableHead>
                <TableHead>Capacity</TableHead>
                <TableHead>Uptime</TableHead>
                <TableHead className='text-right'>Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filteredDeployments.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={9} className='text-center h-24 text-muted-foreground'>
                    No deployments found
                  </TableCell>
                </TableRow>
              ) : (
                filteredDeployments.map((deployment) => {
                  const isDeleting =
                    Boolean(
                      (deployment as { deleted_at?: string | null }).deleted_at ??
                        (deployment as { deletedAt?: string | null }).deletedAt
                    ) ||
                    (deployment as { status?: string }).status === 'deleting'
                  const rawStatus = (deployment as { status?: string }).status ?? 'Unknown'
                  const rawType =
                    (deployment as { type?: string }).type ??
                    (deployment as { kind?: string }).kind ??
                    'Unknown'
                  const statusInfo = statusConfig[deployment.status] ?? {
                    label: rawStatus,
                    color: 'text-gray-600 bg-gray-50',
                    dotColor: 'bg-gray-400',
                  }
                  const typeInfo = typeConfig[deployment.type] ?? {
                    label: rawType,
                    color: 'text-gray-600 bg-gray-50',
                  }
                  const planInfo = DEPLOYMENT_PLANS[deployment.plan] ?? {
                    label: '—',
                    cpu: '—',
                    memory: '—',
                  }

                  return (
                    <TableRow key={deployment.id}>
                      <TableCell>
                        <Link
                          to={`${basePath}/deployments/${deployment.id}`}
                          className='flex flex-col hover:underline'
                        >
                          <span className='font-medium'>{deployment.name}</span>
                          <span className='text-xs text-muted-foreground font-mono'>
                            {deployment.id}
                          </span>
                        </Link>
                      </TableCell>
                      <TableCell>
                        <span className={`inline-flex items-center rounded-md px-2 py-1 text-xs font-medium ${typeInfo.color}`}>
                          {typeInfo.label}
                        </span>
                      </TableCell>
                      <TableCell>
                        <div className='flex items-center gap-2'>
                          <span className={`flex h-2 w-2 rounded-full ${statusInfo.dotColor}`} />
                          <span className={`inline-flex items-center rounded-md px-2 py-1 text-xs font-medium ${statusInfo.color}`}>
                            {statusInfo.label}
                          </span>
                        </div>
                      </TableCell>
                      <TableCell className='text-sm'>{deployment.version}</TableCell>
                      <TableCell className='text-sm'>{deployment.region ?? '—'}</TableCell>
                      <TableCell className='text-sm'>
                        <div className='flex flex-col text-xs'>
                          <span className='font-medium'>{planInfo.label}</span>
                          <span className='text-muted-foreground'>{planInfo.cpu} / {planInfo.memory}</span>
                        </div>
                      </TableCell>
                      <TableCell className='text-sm'>
                         <div className='flex items-center gap-1 text-xs'>
                             <Users className='h-3 w-3 text-muted-foreground' />
                             {deployment.capacity ?? '—'}
                         </div>
                      </TableCell>
                      <TableCell className='text-sm'>
                        {deployment.uptime ?? '—'}
                      </TableCell>
                      <TableCell className='text-right'>
                        <div className='flex items-center justify-end gap-2'>
                          {!isDeleting && deployment.status === 'running' && (
                            <>
                              <Button variant='ghost' size='icon' className='h-8 w-8'>
                                <ExternalLink className='h-4 w-4' />
                              </Button>
                              <Button variant='ghost' size='icon' className='h-8 w-8'>
                                <Square className='h-4 w-4' />
                              </Button>
                            </>
                          )}
                          {!isDeleting && deployment.status === 'stopped' && (
                            <Button variant='ghost' size='icon' className='h-8 w-8'>
                              <Play className='h-4 w-4' />
                            </Button>
                          )}
                          {!isDeleting && (
                            <DropdownMenu>
                              <DropdownMenuTrigger asChild>
                                <Button variant='ghost' size='icon' className='h-8 w-8'>
                                  <MoreVertical className='h-4 w-4' />
                                </Button>
                              </DropdownMenuTrigger>
                              <DropdownMenuContent align='end'>
                                <DropdownMenuItem>
                                  <Copy className='mr-2 h-4 w-4' />
                                  Copy Endpoint
                                </DropdownMenuItem>
                                <DropdownMenuItem>
                                  Settings
                                </DropdownMenuItem>
                                <DropdownMenuItem>
                                  View Logs
                                </DropdownMenuItem>
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
                          {isDeleting && (
                            <span className='text-xs text-muted-foreground'>Deletion in progress</span>
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
