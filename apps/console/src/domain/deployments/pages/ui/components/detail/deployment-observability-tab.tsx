import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { ChartContainer, ChartTooltip, ChartTooltipContent } from '@/components/ui/chart'
import {
  Activity,
  AlertCircle,
  Clock,
  Cpu,
  ExternalLink,
  FileText,
  HardDrive,
  MapPin,
  Package,
  RefreshCw,
  Server,
  Settings,
  Terminal
} from 'lucide-react'
import { Area, AreaChart, CartesianGrid, ReferenceLine, XAxis, YAxis } from 'recharts'

export function DeploymentObservabilityTab() {
  // Mock chart data - CPU Usage over last 24 hours
  const cpuData = [
    { time: '00:00', usage: 32 },
    { time: '02:00', usage: 28 },
    { time: '04:00', usage: 25 },
    { time: '06:00', usage: 35 },
    { time: '08:00', usage: 45 },
    { time: '10:00', usage: 52 },
    { time: '12:00', usage: 48 },
    { time: '14:00', usage: 55 },
    { time: '16:00', usage: 42 },
    { time: '18:00', usage: 38 },
    { time: '20:00', usage: 40 },
    { time: '22:00', usage: 36 },
    { time: 'Now', usage: 42 },
  ]

  // Mock chart data - Memory Usage over last 24 hours
  const memoryData = [
    { time: '00:00', usage: 55 },
    { time: '02:00', usage: 52 },
    { time: '04:00', usage: 50 },
    { time: '06:00', usage: 58 },
    { time: '08:00', usage: 62 },
    { time: '10:00', usage: 68 },
    { time: '12:00', usage: 65 },
    { time: '14:00', usage: 70 },
    { time: '16:00', usage: 66 },
    { time: '18:00', usage: 63 },
    { time: '20:00', usage: 65 },
    { time: '22:00', usage: 62 },
    { time: 'Now', usage: 68 },
  ]

  // Mock instance health data
  const instances = [
    { id: 'inst-1a2b3c', status: 'healthy', cpu: 38, memory: 64, uptime: '15d 4h 23m', region: 'us-east-1a' },
    { id: 'inst-4d5e6f', status: 'healthy', cpu: 45, memory: 72, uptime: '15d 4h 23m', region: 'us-east-1b' },
    { id: 'inst-7g8h9i', status: 'healthy', cpu: 43, memory: 68, uptime: '12d 8h 15m', region: 'us-east-1c' },
  ]

  // Mock instance count progression data
  const instanceCountData = [
    { time: '00:00', count: 2, healthy: 2, unhealthy: 0 },
    { time: '02:00', count: 2, healthy: 2, unhealthy: 0 },
    { time: '04:00', count: 2, healthy: 2, unhealthy: 0 },
    { time: '06:00', count: 3, healthy: 3, unhealthy: 0 },
    { time: '08:00', count: 3, healthy: 3, unhealthy: 0 },
    { time: '10:00', count: 3, healthy: 2, unhealthy: 1 },
    { time: '12:00', count: 3, healthy: 3, unhealthy: 0 },
    { time: '14:00', count: 4, healthy: 4, unhealthy: 0 },
    { time: '16:00', count: 4, healthy: 4, unhealthy: 0 },
    { time: '18:00', count: 3, healthy: 3, unhealthy: 0 },
    { time: '20:00', count: 3, healthy: 3, unhealthy: 0 },
    { time: '22:00', count: 3, healthy: 3, unhealthy: 0 },
    { time: 'Now', count: 3, healthy: 3, unhealthy: 0 },
  ]

  const chartConfig = {
    usage: {
      label: 'Usage',
      color: 'hsl(var(--chart-1))',
    },
  }

  const instanceChartConfig = {
    count: {
      label: 'Total Instances',
      color: 'hsl(221.2 83.2% 53.3%)',
    },
    healthy: {
      label: 'Healthy',
      color: 'hsl(142.1 76.2% 36.3%)',
    },
    unhealthy: {
      label: 'Unhealthy',
      color: 'hsl(0 84.2% 60.2%)',
    },
  }

  return (
    <div className='space-y-6'>
      {/* CPU and Memory Charts */}
      <div className='grid gap-6 md:grid-cols-2'>
        {/* CPU Usage Chart */}
        <Card>
          <CardHeader>
            <CardTitle className='flex items-center gap-2'>
              <Cpu className='h-5 w-5' />
              CPU Usage
            </CardTitle>
            <CardDescription>Last 24 hours - Current: 42%</CardDescription>
          </CardHeader>
          <CardContent>
            <ChartContainer config={chartConfig} className='h-[200px] w-full'>
              <AreaChart data={cpuData}>
                <defs>
                  <linearGradient id='cpuGradient' x1='0' y1='0' x2='0' y2='1'>
                    <stop offset='5%' stopColor='hsl(221.2 83.2% 53.3%)' stopOpacity={0.3} />
                    <stop offset='95%' stopColor='hsl(221.2 83.2% 53.3%)' stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray='3 3' className='stroke-muted' />
                <XAxis
                  dataKey='time'
                  className='text-xs'
                  tickLine={false}
                  axisLine={false}
                />
                <YAxis
                  className='text-xs'
                  tickLine={false}
                  axisLine={false}
                  tickFormatter={(value) => `${value}%`}
                />
                <ChartTooltip content={<ChartTooltipContent />} />
                <Area
                  type='monotone'
                  dataKey='usage'
                  stroke='hsl(221.2 83.2% 53.3%)'
                  fill='url(#cpuGradient)'
                  strokeWidth={2}
                />
              </AreaChart>
            </ChartContainer>
            <div className='mt-4 flex items-center justify-between text-sm'>
              <span className='text-muted-foreground'>Average: 40%</span>
              <span className='text-muted-foreground'>Peak: 55%</span>
            </div>
          </CardContent>
        </Card>

        {/* Memory Usage Chart */}
        <Card>
          <CardHeader>
            <CardTitle className='flex items-center gap-2'>
              <HardDrive className='h-5 w-5' />
              Memory Usage
            </CardTitle>
            <CardDescription>Last 24 hours - Current: 68%</CardDescription>
          </CardHeader>
          <CardContent>
            <ChartContainer config={chartConfig} className='h-[200px] w-full'>
              <AreaChart data={memoryData}>
                <defs>
                  <linearGradient id='memoryGradient' x1='0' y1='0' x2='0' y2='1'>
                    <stop offset='5%' stopColor='hsl(142.1 76.2% 36.3%)' stopOpacity={0.3} />
                    <stop offset='95%' stopColor='hsl(142.1 76.2% 36.3%)' stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray='3 3' className='stroke-muted' />
                <XAxis
                  dataKey='time'
                  className='text-xs'
                  tickLine={false}
                  axisLine={false}
                />
                <YAxis
                  className='text-xs'
                  tickLine={false}
                  axisLine={false}
                  tickFormatter={(value) => `${value}%`}
                />
                <ChartTooltip content={<ChartTooltipContent />} />
                <Area
                  type='monotone'
                  dataKey='usage'
                  stroke='hsl(142.1 76.2% 36.3%)'
                  fill='url(#memoryGradient)'
                  strokeWidth={2}
                />
              </AreaChart>
            </ChartContainer>
            <div className='mt-4 flex items-center justify-between text-sm'>
              <span className='text-muted-foreground'>Average: 62%</span>
              <span className='text-muted-foreground'>Peak: 70%</span>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Instance Health Chart */}
      <Card>
        <CardHeader>
          <CardTitle className='flex items-center gap-2'>
            <Server className='h-5 w-5' />
            Instance Count & Health
          </CardTitle>
          <CardDescription>Last 24 hours - Current: {instances.length} instances (Max: 5)</CardDescription>
        </CardHeader>
        <CardContent>
          <ChartContainer config={instanceChartConfig} className='h-[250px] w-full'>
            <AreaChart data={instanceCountData}>
              <defs>
                <linearGradient id='instanceGradient' x1='0' y1='0' x2='0' y2='1'>
                  <stop offset='5%' stopColor='hsl(221.2 83.2% 53.3%)' stopOpacity={0.3} />
                  <stop offset='95%' stopColor='hsl(221.2 83.2% 53.3%)' stopOpacity={0.05} />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray='3 3' className='stroke-muted' />
              <XAxis
                dataKey='time'
                className='text-xs'
                tickLine={false}
                axisLine={false}
              />
              <YAxis
                className='text-xs'
                tickLine={false}
                axisLine={false}
                domain={[0, 6]}
              />
              <ChartTooltip content={<ChartTooltipContent />} />

              {/* Reference line for instance limit */}
              <ReferenceLine
                y={5}
                stroke='hsl(0 84.2% 60.2%)'
                strokeDasharray='5 5'
                strokeWidth={2}
                label={{
                  value: 'Max Instances',
                  position: 'right',
                  fill: 'hsl(0 84.2% 60.2%)',
                  fontSize: 12
                }}
              />

              <Area
                type='linear'
                dataKey='count'
                stroke='hsl(221.2 83.2% 53.3%)'
                fill='url(#instanceGradient)'
                strokeWidth={2}
              />
            </AreaChart>
          </ChartContainer>
          <div className='mt-6 grid gap-4 md:grid-cols-3'>
            <div className='flex items-center gap-3'>
              <div className='h-3 w-3 rounded-full bg-blue-500' />
              <div>
                <p className='text-xs text-muted-foreground'>Current Instances</p>
                <p className='text-xl font-bold'>{instances.length}</p>
              </div>
            </div>
            <div className='flex items-center gap-3'>
              <div className='h-3 w-3 rounded-full bg-green-500' />
              <div>
                <p className='text-xs text-muted-foreground'>Healthy</p>
                <p className='text-xl font-bold text-green-600'>{instances.length}</p>
              </div>
            </div>
            <div className='flex items-center gap-3'>
              <div className='h-3 w-3 rounded-full border-2 border-dashed border-red-500' />
              <div>
                <p className='text-xs text-muted-foreground'>Limit</p>
                <p className='text-xl font-bold'>5</p>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Individual Instance Details */}
      <Card>
        <CardHeader>
          <CardTitle>Instance Details</CardTitle>
          <CardDescription>Individual instance metrics and status</CardDescription>
        </CardHeader>
        <CardContent>
          <div className='space-y-4'>
            {instances.map((instance) => (
              <div key={instance.id} className='border rounded-lg p-4'>
                <div className='flex items-center justify-between mb-3'>
                  <div className='flex items-center gap-3'>
                    <div className='flex items-center gap-2'>
                      <div className='h-2 w-2 rounded-full bg-green-500' />
                      <span className='font-mono text-sm font-medium'>{instance.id}</span>
                    </div>
                    <span className='inline-flex items-center rounded-md px-2 py-1 text-xs font-medium text-green-600 bg-green-50'>
                      Healthy
                    </span>
                  </div>
                  <div className='flex items-center gap-4 text-sm text-muted-foreground'>
                    <div className='flex items-center gap-1'>
                      <MapPin className='h-3 w-3' />
                      <span>{instance.region}</span>
                    </div>
                    <div className='flex items-center gap-1'>
                      <Clock className='h-3 w-3' />
                      <span>{instance.uptime}</span>
                    </div>
                  </div>
                </div>
                <div className='grid grid-cols-2 gap-4'>
                  <div className='space-y-2'>
                    <div className='flex items-center justify-between'>
                      <span className='text-xs text-muted-foreground'>CPU</span>
                      <span className='text-xs font-bold'>{instance.cpu}%</span>
                    </div>
                    <div className='h-1.5 bg-muted rounded-full overflow-hidden'>
                      <div
                        className='h-full bg-blue-500 transition-all'
                        style={{ width: `${instance.cpu}%` }}
                      />
                    </div>
                  </div>
                  <div className='space-y-2'>
                    <div className='flex items-center justify-between'>
                      <span className='text-xs text-muted-foreground'>Memory</span>
                      <span className='text-xs font-bold'>{instance.memory}%</span>
                    </div>
                    <div className='h-1.5 bg-muted rounded-full overflow-hidden'>
                      <div
                        className='h-full bg-green-500 transition-all'
                        style={{ width: `${instance.memory}%` }}
                      />
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Network Metrics */}
      <Card>
        <CardHeader>
          <CardTitle className='flex items-center gap-2'>
            <Activity className='h-5 w-5' />
            Network Metrics
          </CardTitle>
          <CardDescription>Real-time network I/O</CardDescription>
        </CardHeader>
        <CardContent>
          <div className='grid gap-6 md:grid-cols-3'>
            <div className='space-y-2'>
              <div className='flex items-center justify-between'>
                <span className='text-sm text-muted-foreground'>Bandwidth In</span>
                <span className='text-sm font-bold'>80 MB/s</span>
              </div>
              <div className='h-2 bg-muted rounded-full overflow-hidden'>
                <div className='h-full bg-purple-500' style={{ width: '40%' }} />
              </div>
            </div>
            <div className='space-y-2'>
              <div className='flex items-center justify-between'>
                <span className='text-sm text-muted-foreground'>Bandwidth Out</span>
                <span className='text-sm font-bold'>45 MB/s</span>
              </div>
              <div className='h-2 bg-muted rounded-full overflow-hidden'>
                <div className='h-full bg-orange-500' style={{ width: '25%' }} />
              </div>
            </div>
            <div className='space-y-2'>
              <div className='flex items-center justify-between'>
                <span className='text-sm text-muted-foreground'>Total I/O</span>
                <span className='text-sm font-bold'>125 MB/s</span>
              </div>
              <div className='h-2 bg-muted rounded-full overflow-hidden'>
                <div className='h-full bg-indigo-500' style={{ width: '35%' }} />
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Logs */}
      <Card>
        <CardHeader>
          <div className='flex items-center justify-between'>
            <div>
              <CardTitle className='flex items-center gap-2'>
                <Terminal className='h-5 w-5' />
                Application Logs
              </CardTitle>
              <CardDescription>Recent logs from your deployment</CardDescription>
            </div>
            <Button variant='outline' size='sm'>
              <ExternalLink className='mr-2 h-4 w-4' />
              View Full Logs
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <div className='bg-slate-950 text-slate-50 rounded-lg p-4 font-mono text-xs space-y-1 max-h-[400px] overflow-y-auto'>
            <div className='text-green-400'>[2026-01-08 10:23:45] INFO: Deployment started successfully</div>
            <div className='text-blue-400'>[2026-01-08 10:23:46] DEBUG: Loading configuration from /etc/config</div>
            <div className='text-slate-400'>[2026-01-08 10:23:47] INFO: Database connection established</div>
            <div className='text-slate-400'>[2026-01-08 10:23:48] INFO: Starting HTTP server on port 8080</div>
            <div className='text-green-400'>[2026-01-08 10:23:49] INFO: Server is ready to accept connections</div>
            <div className='text-slate-400'>[2026-01-08 10:24:15] INFO: Received authentication request from 192.168.1.1</div>
            <div className='text-slate-400'>[2026-01-08 10:24:15] INFO: User authenticated successfully</div>
            <div className='text-yellow-400'>[2026-01-08 10:24:30] WARN: High memory usage detected: 68%</div>
            <div className='text-slate-400'>[2026-01-08 10:25:00] INFO: Health check passed</div>
            <div className='text-slate-400'>[2026-01-08 10:25:30] INFO: Processed 150 requests in the last minute</div>
          </div>
        </CardContent>
      </Card>

      {/* Events */}
      <Card>
        <CardHeader>
          <CardTitle className='flex items-center gap-2'>
            <FileText className='h-5 w-5' />
            Recent Events
          </CardTitle>
          <CardDescription>System events and deployment activities</CardDescription>
        </CardHeader>
        <CardContent>
          <div className='space-y-4'>
            {[
              { time: '5 minutes ago', type: 'success', message: 'Health check passed', icon: Activity },
              { time: '15 minutes ago', type: 'warning', message: 'High CPU usage detected', icon: AlertCircle },
              { time: '1 hour ago', type: 'info', message: 'Configuration updated', icon: Settings },
              { time: '2 hours ago', type: 'success', message: 'Deployment restarted', icon: RefreshCw },
              { time: '1 day ago', type: 'success', message: 'New version deployed: 23.0.1', icon: Package },
            ].map((event, index) => (
              <div key={index} className='flex items-start gap-3 pb-3 border-b last:border-0 last:pb-0'>
                <div className={`p-2 rounded-full ${event.type === 'success' ? 'bg-green-100 text-green-600' :
                  event.type === 'warning' ? 'bg-yellow-100 text-yellow-600' :
                    'bg-blue-100 text-blue-600'
                  }`}>
                  <event.icon className='h-4 w-4' />
                </div>
                <div className='flex-1'>
                  <p className='text-sm font-medium'>{event.message}</p>
                  <p className='text-xs text-muted-foreground'>{event.time}</p>
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
