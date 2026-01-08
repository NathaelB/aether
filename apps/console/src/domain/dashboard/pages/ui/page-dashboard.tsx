import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Activity, Server, ShieldCheck, AlertTriangle } from 'lucide-react'

export const PageDashboard = () => {
  return (
    <div className='space-y-6'>
      <div className='grid gap-4 md:grid-cols-2 lg:grid-cols-4'>
        <Card>
          <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
            <CardTitle className='text-sm font-medium'>
              Total Instances
            </CardTitle>
            <Server className='h-4 w-4 text-muted-foreground' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>12</div>
            <p className='text-xs text-muted-foreground'>
              +2 from last month
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
            <CardTitle className='text-sm font-medium'>
              Healthy
            </CardTitle>
            <ShieldCheck className='h-4 w-4 text-muted-foreground' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>10</div>
            <p className='text-xs text-muted-foreground'>
              83% uptime
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
            <CardTitle className='text-sm font-medium'>
              Warnings
            </CardTitle>
            <AlertTriangle className='h-4 w-4 text-muted-foreground' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>2</div>
            <p className='text-xs text-muted-foreground'>
              High memory usage
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className='flex flex-row items-center justify-between space-y-0 pb-2'>
            <CardTitle className='text-sm font-medium'>
              Active Deployments
            </CardTitle>
            <Activity className='h-4 w-4 text-muted-foreground' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>+573</div>
            <p className='text-xs text-muted-foreground'>
              +201 since last hour
            </p>
          </CardContent>
        </Card>
      </div>
      
      <div className='grid gap-4 md:grid-cols-2 lg:grid-cols-7'>
        <Card className='col-span-4'>
          <CardHeader>
            <CardTitle>Overview</CardTitle>
          </CardHeader>
          <CardContent className='pl-2'>
             <div className='h-[200px] flex items-center justify-center text-muted-foreground'>
                Chart Placeholder
             </div>
          </CardContent>
        </Card>
        <Card className='col-span-3'>
          <CardHeader>
            <CardTitle>Recent Activity</CardTitle>
          </CardHeader>
          <CardContent>
            <div className='space-y-8'>
              <div className='flex items-center'>
                <div className='ml-4 space-y-1'>
                  <p className='text-sm font-medium leading-none'>
                    Instance #492 created
                  </p>
                  <p className='text-sm text-muted-foreground'>
                    Just now
                  </p>
                </div>
                <div className='ml-auto font-medium'>
                    +1
                </div>
              </div>
              <div className='flex items-center'>
                <div className='ml-4 space-y-1'>
                  <p className='text-sm font-medium leading-none'>
                    Deployment failed
                  </p>
                  <p className='text-sm text-muted-foreground'>
                    2 hours ago
                  </p>
                </div>
                <div className='ml-auto font-medium text-destructive'>
                    -1
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
