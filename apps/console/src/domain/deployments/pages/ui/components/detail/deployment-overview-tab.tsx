import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import {
  Calendar,
  Clock,
  Copy,
  Cpu,
  ExternalLink,
  HardDrive,
  MapPin,
  Package,
  Server,
  Users
} from 'lucide-react'
import { Deployment, DEPLOYMENT_PLANS, DeploymentType } from '../../../../types/deployment'

interface DeploymentOverviewTabProps {
  deployment: Deployment
}

const typeConfig: Record<DeploymentType, { label: string; color: string }> = {
  keycloak: { label: 'Keycloak', color: 'text-blue-700 bg-blue-100' },
  ferriskey: { label: 'Ferriskey', color: 'text-purple-700 bg-purple-100' },
  authentik: { label: 'Authentik', color: 'text-orange-700 bg-orange-100' },
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
  const typeInfo = typeConfig[deployment.type]
  const planInfo = DEPLOYMENT_PLANS[deployment.plan]

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
              <span>Type</span>
            </div>
            <span className={`inline-flex items-center rounded-md px-2 py-1 text-xs font-medium ${typeInfo.color}`}>
              {typeInfo.label}
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
              <MapPin className='h-4 w-4' />
              <span>Region</span>
            </div>
            <span className='text-sm font-medium'>{deployment.region}</span>
          </div>
          <Separator />
          <div className='flex items-center justify-between'>
            <div className='flex items-center gap-2 text-sm text-muted-foreground'>
              <Server className='h-4 w-4' />
              <span>Environment</span>
            </div>
            <span className='text-sm font-medium capitalize'>{deployment.environment}</span>
          </div>
          <Separator />
          <div className='flex items-center justify-between'>
            <div className='flex items-center gap-2 text-sm text-muted-foreground'>
              <Calendar className='h-4 w-4' />
              <span>Created</span>
            </div>
            <span className='text-sm font-medium'>{formatDate(deployment.createdAt)}</span>
          </div>
          <Separator />
          <div className='flex items-center justify-between'>
            <div className='flex items-center gap-2 text-sm text-muted-foreground'>
              <Clock className='h-4 w-4' />
              <span>Last Deployment</span>
            </div>
            <span className='text-sm font-medium'>{formatDate(deployment.lastDeployment)}</span>
          </div>
        </CardContent>
      </Card>

      {/* Resource Allocation & Endpoint */}
      <div className='space-y-6'>
        <Card>
          <CardHeader>
            <CardTitle>Resource Allocation</CardTitle>
            <CardDescription>Plan and resource configuration</CardDescription>
          </CardHeader>
          <CardContent className='space-y-4'>
            <div className='flex items-center justify-between'>
              <div className='text-sm text-muted-foreground'>Plan</div>
              <div className='text-right'>
                <div className='text-sm font-medium'>{planInfo.label}</div>
                <div className='text-xs text-muted-foreground'>{planInfo.description}</div>
              </div>
            </div>
            <Separator />
            <div className='flex items-center justify-between'>
              <div className='flex items-center gap-2 text-sm text-muted-foreground'>
                <Cpu className='h-4 w-4' />
                <span>CPU</span>
              </div>
              <span className='text-sm font-medium'>{planInfo.cpu}</span>
            </div>
            <Separator />
            <div className='flex items-center justify-between'>
              <div className='flex items-center gap-2 text-sm text-muted-foreground'>
                <HardDrive className='h-4 w-4' />
                <span>Memory</span>
              </div>
              <span className='text-sm font-medium'>{planInfo.memory}</span>
            </div>
            <Separator />
            <div className='flex items-center justify-between'>
              <div className='flex items-center gap-2 text-sm text-muted-foreground'>
                <Server className='h-4 w-4' />
                <span>Max Realms</span>
              </div>
              <span className='text-sm font-medium'>{planInfo.maxRealms}</span>
            </div>
            <Separator />
            <div className='flex items-center justify-between'>
              <div className='flex items-center gap-2 text-sm text-muted-foreground'>
                <Users className='h-4 w-4' />
                <span>User Capacity</span>
              </div>
              <span className='text-sm font-medium'>{deployment.capacity.toLocaleString()}</span>
            </div>
          </CardContent>
        </Card>

        {deployment.endpoint && (
          <Card>
            <CardHeader>
              <CardTitle>Endpoint</CardTitle>
              <CardDescription>Access URL for this deployment</CardDescription>
            </CardHeader>
            <CardContent>
              <div className='flex items-center justify-between p-3 bg-muted rounded-md'>
                <code className='text-sm font-mono'>{deployment.endpoint}</code>
                <div className='flex items-center gap-2'>
                  <Button
                    variant='ghost'
                    size='icon'
                    className='h-8 w-8'
                    onClick={() => copyToClipboard(deployment.endpoint!)}
                  >
                    <Copy className='h-4 w-4' />
                  </Button>
                  <Button
                    variant='ghost'
                    size='icon'
                    className='h-8 w-8'
                    onClick={() => window.open(deployment.endpoint, '_blank')}
                  >
                    <ExternalLink className='h-4 w-4' />
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  )
}
