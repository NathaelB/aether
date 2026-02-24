import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Activity, Clock, GitCommit } from 'lucide-react'
import { Deployment, DeploymentStatus } from '../../../../types/deployment'

interface DeploymentStatusCardsProps {
  deployment: Deployment
}

const statusConfig: Record<DeploymentStatus, { label: string; color: string; dotColor: string }> = {
  pending: { label: 'Pending', color: 'text-gray-600 bg-gray-50', dotColor: 'bg-gray-400' },
  scheduling: { label: 'Scheduling', color: 'text-blue-600 bg-blue-50', dotColor: 'bg-blue-400' },
  in_progress: { label: 'In Progress', color: 'text-blue-600 bg-blue-50', dotColor: 'bg-blue-500' },
  successful: { label: 'Successful', color: 'text-green-600 bg-green-50', dotColor: 'bg-green-500' },
  failed: { label: 'Failed', color: 'text-red-600 bg-red-50', dotColor: 'bg-red-500' },
  maintenance: { label: 'Maintenance', color: 'text-yellow-600 bg-yellow-50', dotColor: 'bg-yellow-500' },
  upgrade_required: { label: 'Upgrade Required', color: 'text-orange-600 bg-orange-50', dotColor: 'bg-orange-500' },
  upgrading: { label: 'Upgrading', color: 'text-purple-600 bg-purple-50', dotColor: 'bg-purple-500' },
}

export function DeploymentStatusCards({ deployment }: DeploymentStatusCardsProps) {
  const statusInfo = statusConfig[deployment.status] ?? { label: deployment.status, color: 'text-gray-600 bg-gray-50', dotColor: 'bg-gray-400' }

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
