import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import {
  Calendar,
  Clock,
  Copy,
  Package,
  Server,
} from 'lucide-react'
import { Deployment, DeploymentKind } from '../../../../types/deployment'

interface DeploymentOverviewTabProps {
  deployment: Deployment
}

const kindConfig: Record<DeploymentKind, { label: string; color: string }> = {
  keycloak: { label: 'Keycloak', color: 'text-blue-700 bg-blue-100' },
  ferriskey: { label: 'Ferriskey', color: 'text-purple-700 bg-purple-100' },
}

const formatDate = (dateString: string) => {
  return new Date(dateString).toLocaleString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

export function DeploymentOverviewTab({ deployment }: DeploymentOverviewTabProps) {
  const kindInfo = kindConfig[deployment.kind] ?? {
    label: deployment.kind,
    color: 'text-gray-600 bg-gray-50',
  }

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text)
  }

  return (
    <div className='grid gap-6 md:grid-cols-2'>
      {/* Deployment Information */}
      <Card>
        <CardHeader>
          <CardTitle>Deployment Information</CardTitle>
          <CardDescription>General information about this deployment</CardDescription>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='flex items-center justify-between'>
            <div className='flex items-center gap-2 text-sm text-muted-foreground'>
              <Server className='h-4 w-4' />
              <span>Kind</span>
            </div>
            <span className={`inline-flex items-center rounded-md px-2 py-1 text-xs font-medium ${kindInfo.color}`}>
              {kindInfo.label}
            </span>
          </div>
          <Separator />
          <div className='flex items-center justify-between'>
            <div className='flex items-center gap-2 text-sm text-muted-foreground'>
              <Package className='h-4 w-4' />
              <span>Version</span>
            </div>
            <span className='text-sm font-medium'>{deployment.version}</span>
          </div>
          <Separator />
          <div className='flex items-center justify-between'>
            <div className='flex items-center gap-2 text-sm text-muted-foreground'>
              <Server className='h-4 w-4' />
              <span>Namespace</span>
            </div>
            <span className='text-sm font-medium font-mono'>{deployment.namespace}</span>
          </div>
          <Separator />
          <div className='flex items-center justify-between'>
            <div className='flex items-center gap-2 text-sm text-muted-foreground'>
              <Calendar className='h-4 w-4' />
              <span>Created</span>
            </div>
            <span className='text-sm font-medium'>{formatDate(deployment.created_at)}</span>
          </div>
          <Separator />
          <div className='flex items-center justify-between'>
            <div className='flex items-center gap-2 text-sm text-muted-foreground'>
              <Clock className='h-4 w-4' />
              <span>Last Deployed</span>
            </div>
            <span className='text-sm font-medium'>
              {deployment.deployed_at ? formatDate(deployment.deployed_at) : '—'}
            </span>
          </div>
        </CardContent>
      </Card>

      {/* Identifiers */}
      <Card>
        <CardHeader>
          <CardTitle>Identifiers</CardTitle>
          <CardDescription>Resource identifiers for this deployment</CardDescription>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='flex items-center justify-between'>
            <div className='text-sm text-muted-foreground'>Deployment ID</div>
            <div className='flex items-center gap-2'>
              <code className='text-xs font-mono'>{deployment.id}</code>
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
          <Separator />
          <div className='flex items-center justify-between'>
            <div className='text-sm text-muted-foreground'>Organisation ID</div>
            <div className='flex items-center gap-2'>
              <code className='text-xs font-mono'>{deployment.organisation_id}</code>
              <Button
                variant='ghost'
                size='icon'
                className='h-6 w-6'
                onClick={() => copyToClipboard(deployment.organisation_id)}
              >
                <Copy className='h-3 w-3' />
              </Button>
            </div>
          </div>
          <Separator />
          <div className='flex items-center justify-between'>
            <div className='text-sm text-muted-foreground'>Dataplane ID</div>
            <div className='flex items-center gap-2'>
              <code className='text-xs font-mono'>{deployment.dataplane_id}</code>
              <Button
                variant='ghost'
                size='icon'
                className='h-6 w-6'
                onClick={() => copyToClipboard(deployment.dataplane_id)}
              >
                <Copy className='h-3 w-3' />
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
