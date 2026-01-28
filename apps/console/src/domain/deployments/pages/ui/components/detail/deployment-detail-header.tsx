import { Button } from '@/components/ui/button'
import {
  ArrowLeft,
  Copy,
  ExternalLink,
  Play,
  RefreshCw,
  Settings,
  Square,
  Trash2,
} from 'lucide-react'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Deployment, DeploymentType } from '../../../../types/deployment'

interface DeploymentDetailHeaderProps {
  deployment: Deployment
  onBack: () => void
  onRefresh: () => void
}

const typeConfig: Record<DeploymentType, { label: string; color: string }> = {
  keycloak: { label: 'Keycloak', color: 'text-blue-700 bg-blue-100' },
  ferriskey: { label: 'Ferriskey', color: 'text-purple-700 bg-purple-100' },
  authentik: { label: 'Authentik', color: 'text-orange-700 bg-orange-100' },
}

export function DeploymentDetailHeader({ deployment, onBack, onRefresh }: DeploymentDetailHeaderProps) {
  const isDeleting =
    Boolean(
      (deployment as { deleted_at?: string | null }).deleted_at ??
        (deployment as { deletedAt?: string | null }).deletedAt
    ) ||
    (deployment as { status?: string }).status === 'deleting'
  const rawType =
    (deployment as { type?: string }).type ??
    (deployment as { kind?: string }).kind ??
    'Unknown'
  const typeInfo = typeConfig[deployment.type] ?? {
    label: rawType,
    color: 'text-gray-600 bg-gray-50',
  }

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text)
  }

  return (
    <div className='flex items-center justify-between'>
      <div className='flex items-center gap-4'>
        <Button variant='ghost' size='icon' onClick={onBack}>
          <ArrowLeft className='h-4 w-4' />
        </Button>
        <div>
          <div className='flex items-center gap-3'>
            <h2 className='text-2xl font-bold tracking-tight'>{deployment.name}</h2>
            <span className={`inline-flex items-center rounded-md px-2 py-1 text-xs font-medium ${typeInfo.color}`}>
              {typeInfo.label}
            </span>
            {isDeleting && (
              <span className='inline-flex items-center rounded-md bg-amber-50 px-2 py-1 text-xs font-medium text-amber-700'>
                Deleting
              </span>
            )}
          </div>
          <div className='flex items-center gap-2 mt-1'>
            <span className='text-sm text-muted-foreground font-mono'>{deployment.id}</span>
            <Button
              variant='ghost'
              size='icon'
              className='h-6 w-6'
              onClick={() => copyToClipboard(deployment.id)}
            >
              <Copy className='h-3 w-3' />
            </Button>
          </div>
        </div>
      </div>
      <div className='flex items-center gap-2'>
        {!isDeleting && deployment.status === 'running' && (
          <>
            {deployment.endpoint && (
              <Button variant='outline' className='gap-2' onClick={() => window.open(deployment.endpoint, '_blank')}>
                <ExternalLink className='h-4 w-4' />
                Open Console
              </Button>
            )}
            <Button variant='outline' size='icon'>
              <Square className='h-4 w-4' />
            </Button>
          </>
        )}
        {!isDeleting && deployment.status === 'stopped' && (
          <Button variant='outline' className='gap-2'>
            <Play className='h-4 w-4' />
            Start
          </Button>
        )}
        <Button variant='outline' size='icon' onClick={onRefresh}>
          <RefreshCw className='h-4 w-4' />
        </Button>
        {!isDeleting && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant='outline'>
                <Settings className='mr-2 h-4 w-4' />
                Actions
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align='end'>
              <DropdownMenuItem>Restart Deployment</DropdownMenuItem>
              <DropdownMenuItem>View Logs</DropdownMenuItem>
              <DropdownMenuItem>Download Backup</DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem className='text-red-600'>
                <Trash2 className='mr-2 h-4 w-4' />
                Delete Deployment
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        )}
        {isDeleting && (
          <span className='text-xs text-muted-foreground'>Deletion in progress</span>
        )}
      </div>
    </div>
  )
}
