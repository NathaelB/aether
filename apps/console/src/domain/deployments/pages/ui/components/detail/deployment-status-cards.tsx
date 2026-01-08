import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Activity, Clock, Users } from 'lucide-react'
import { Deployment, DeploymentStatus } from '../../../../types/deployment'

interface DeploymentStatusCardsProps {
  deployment: Deployment
}

const statusConfig: Record<DeploymentStatus, { label: string; color: string; dotColor: string }> = {
  running: { label: 'Running', color: 'text-green-600 bg-green-50', dotColor: 'bg-green-500' },
  stopped: { label: 'Stopped', color: 'text-gray-600 bg-gray-50', dotColor: 'bg-gray-400' },
  deploying: { label: 'Deploying', color: 'text-blue-600 bg-blue-50', dotColor: 'bg-blue-500' },
  maintenance: { label: 'Maintenance', color: 'text-yellow-600 bg-yellow-50', dotColor: 'bg-yellow-500' },
  error: { label: 'Error', color: 'text-red-600 bg-red-50', dotColor: 'bg-red-500' },
}

export function DeploymentStatusCards({ deployment }: DeploymentStatusCardsProps) {
  const statusInfo = statusConfig[deployment.status]

  return (
    <div className='grid gap-4 md:grid-cols-3'>
      <Card>
        <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
          <CardTitle className='text-sm font-medium'>Status</CardTitle>
          <Activity className='h-4 w-4 text-muted-foreground' />
        </CardHeader>
        <CardContent>
          <div className='flex items-center gap-2'>
            <span className={`flex h-2 w-2 rounded-full ${statusInfo.dotColor}`} />
            <span className={`inline-flex items-center rounded-md px-2 py-1 text-xs font-medium ${statusInfo.color}`}>
              {statusInfo.label}
            </span>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
          <CardTitle className='text-sm font-medium'>Uptime</CardTitle>
          <Clock className='h-4 w-4 text-muted-foreground' />
        </CardHeader>
        <CardContent>
          <div className='text-2xl font-bold'>{deployment.uptime || 'N/A'}</div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
          <CardTitle className='text-sm font-medium'>Capacity</CardTitle>
          <Users className='h-4 w-4 text-muted-foreground' />
        </CardHeader>
        <CardContent>
          <div className='text-2xl font-bold'>{deployment.capacity.toLocaleString()}</div>
          <p className='text-xs text-muted-foreground mt-1'>active users</p>
        </CardContent>
      </Card>
    </div>
  )
}
