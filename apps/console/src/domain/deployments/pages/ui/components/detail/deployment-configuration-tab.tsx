import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { Copy, Trash2 } from 'lucide-react'
import { Deployment, DEPLOYMENT_PLANS } from '../../../../types/deployment'

interface DeploymentConfigurationTabProps {
  deployment: Deployment
}

export function DeploymentConfigurationTab({ deployment }: DeploymentConfigurationTabProps) {
  const planInfo = DEPLOYMENT_PLANS[deployment.plan]

  return (
    <div className='space-y-6'>
      {/* General Settings */}
      <Card>
        <CardHeader>
          <CardTitle>General Settings</CardTitle>
          <CardDescription>Configure deployment parameters</CardDescription>
        </CardHeader>
        <CardContent className='space-y-6'>
          <div className='space-y-2'>
            <Label htmlFor='deployment-name'>Deployment Name</Label>
            <Input id='deployment-name' defaultValue={deployment.name} />
          </div>
          <div className='space-y-2'>
            <Label htmlFor='deployment-version'>Version</Label>
            <Input id='deployment-version' defaultValue={deployment.version} disabled />
            <p className='text-xs text-muted-foreground'>To update version, deploy a new version from the overview</p>
          </div>
          <div className='flex items-center justify-between'>
            <div className='space-y-0.5'>
              <Label>Auto-scaling</Label>
              <p className='text-sm text-muted-foreground'>Automatically adjust resources based on load</p>
            </div>
            <Switch defaultChecked />
          </div>
          <div className='flex items-center justify-between'>
            <div className='space-y-0.5'>
              <Label>High Availability</Label>
              <p className='text-sm text-muted-foreground'>Deploy across multiple availability zones</p>
            </div>
            <Switch />
          </div>
          <Separator />
          <Button>Save Changes</Button>
        </CardContent>
      </Card>

      {/* Environment Variables */}
      <Card>
        <CardHeader>
          <div className='flex items-center justify-between'>
            <div>
              <CardTitle>Environment Variables</CardTitle>
              <CardDescription>Manage deployment environment variables</CardDescription>
            </div>
            <Button variant='outline' size='sm'>
              Add Variable
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <div className='space-y-3'>
            {[
              { key: 'DB_HOST', value: 'postgres.internal', secret: false },
              { key: 'DB_PORT', value: '5432', secret: false },
              { key: 'DB_PASSWORD', value: '••••••••', secret: true },
              { key: 'REDIS_URL', value: 'redis://cache.internal:6379', secret: false },
              { key: 'API_KEY', value: '••••••••••••', secret: true },
            ].map((env, index) => (
              <div key={index} className='flex items-center gap-2 p-3 border rounded-lg'>
                <div className='flex-1 grid grid-cols-2 gap-4'>
                  <div>
                    <p className='text-xs text-muted-foreground mb-1'>Key</p>
                    <p className='text-sm font-mono font-medium'>{env.key}</p>
                  </div>
                  <div>
                    <p className='text-xs text-muted-foreground mb-1'>Value</p>
                    <p className='text-sm font-mono'>{env.value}</p>
                  </div>
                </div>
                <Button variant='ghost' size='icon' className='h-8 w-8'>
                  <Copy className='h-4 w-4' />
                </Button>
                <Button variant='ghost' size='icon' className='h-8 w-8'>
                  <Trash2 className='h-4 w-4 text-red-600' />
                </Button>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Network Configuration */}
      <Card>
        <CardHeader>
          <CardTitle>Network Configuration</CardTitle>
          <CardDescription>Manage network settings and endpoints</CardDescription>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='space-y-2'>
            <Label htmlFor='endpoint'>Public Endpoint</Label>
            <div className='flex gap-2'>
              <Input id='endpoint' defaultValue={deployment.endpoint} className='flex-1' />
              <Button variant='outline' size='icon'>
                <Copy className='h-4 w-4' />
              </Button>
            </div>
          </div>
          <div className='flex items-center justify-between'>
            <div className='space-y-0.5'>
              <Label>TLS/SSL</Label>
              <p className='text-sm text-muted-foreground'>Enable HTTPS encryption</p>
            </div>
            <Switch defaultChecked />
          </div>
          <div className='flex items-center justify-between'>
            <div className='space-y-0.5'>
              <Label>Custom Domain</Label>
              <p className='text-sm text-muted-foreground'>Use your own domain name</p>
            </div>
            <Button variant='outline' size='sm'>Configure</Button>
          </div>
        </CardContent>
      </Card>

      {/* Resource Limits */}
      <Card>
        <CardHeader>
          <CardTitle>Resource Limits</CardTitle>
          <CardDescription>Current plan: {planInfo.label}</CardDescription>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='grid gap-4 md:grid-cols-2'>
            <div className='space-y-2'>
              <Label>CPU Allocation</Label>
              <div className='flex items-center justify-between p-3 bg-muted rounded-md'>
                <span className='text-sm'>{planInfo.cpu}</span>
                <Button variant='ghost' size='sm'>Upgrade</Button>
              </div>
            </div>
            <div className='space-y-2'>
              <Label>Memory Allocation</Label>
              <div className='flex items-center justify-between p-3 bg-muted rounded-md'>
                <span className='text-sm'>{planInfo.memory}</span>
                <Button variant='ghost' size='sm'>Upgrade</Button>
              </div>
            </div>
          </div>
          <Separator />
          <Button variant='outline' className='w-full'>View All Plans</Button>
        </CardContent>
      </Card>
    </div>
  )
}
