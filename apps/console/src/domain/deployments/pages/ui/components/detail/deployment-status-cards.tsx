import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Activity, Clock, GitCommit } from 'lucide-react'
import { Deployment } from '../../../../types/deployment'
import { DeploymentStatusBadge } from '../deployment-status'

interface DeploymentStatusCardsProps {
  deployment: Deployment
}

export function DeploymentStatusCards({ deployment }: DeploymentStatusCardsProps) {

  return (
    <div className='grid gap-4 md:grid-cols-3'>
      <Card>
        <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
          <CardTitle className='text-sm font-medium'>Status</CardTitle>
          <Activity className='h-4 w-4 text-muted-foreground' />
        </CardHeader>
        <CardContent>
          <div className='flex items-center gap-2'>
            <DeploymentStatusBadge status={deployment.status} />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
          <CardTitle className='text-sm font-medium'>Deployed</CardTitle>
          <Clock className='h-4 w-4 text-muted-foreground' />
        </CardHeader>
        <CardContent>
          <div className='text-2xl font-bold'>
            {deployment.deployed_at ? new Date(deployment.deployed_at).toLocaleDateString() : 'N/A'}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
          <CardTitle className='text-sm font-medium'>Version</CardTitle>
          <GitCommit className='h-4 w-4 text-muted-foreground' />
        </CardHeader>
        <CardContent>
          <div className='text-2xl font-bold'>{deployment.version}</div>
        </CardContent>
      </Card>
    </div>
  )
}
