import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import { AlertCircle, BarChart3, Cpu, TrendingUp, Zap } from 'lucide-react'

export function DeploymentInsightsTab() {
  return (
    <div className='space-y-6'>
      {/* Performance Analytics */}
      <Card>
        <CardHeader>
          <CardTitle className='flex items-center gap-2'>
            <TrendingUp className='h-5 w-5' />
            Performance Analytics
          </CardTitle>
          <CardDescription>24-hour performance overview</CardDescription>
        </CardHeader>
        <CardContent>
          <div className='grid gap-6 md:grid-cols-2'>
            <div>
              <h4 className='text-sm font-semibold mb-4'>Response Time</h4>
              <div className='space-y-3'>
                <div>
                  <div className='flex items-center justify-between mb-1'>
                    <span className='text-sm text-muted-foreground'>Average</span>
                    <span className='text-sm font-bold'>245ms</span>
                  </div>
                  <div className='h-2 bg-muted rounded-full overflow-hidden'>
                    <div className='h-full bg-green-500' style={{ width: '82%' }} />
                  </div>
                </div>
                <div>
                  <div className='flex items-center justify-between mb-1'>
                    <span className='text-sm text-muted-foreground'>P95</span>
                    <span className='text-sm font-bold'>580ms</span>
                  </div>
                  <div className='h-2 bg-muted rounded-full overflow-hidden'>
                    <div className='h-full bg-yellow-500' style={{ width: '58%' }} />
                  </div>
                </div>
                <div>
                  <div className='flex items-center justify-between mb-1'>
                    <span className='text-sm text-muted-foreground'>P99</span>
                    <span className='text-sm font-bold'>1.2s</span>
                  </div>
                  <div className='h-2 bg-muted rounded-full overflow-hidden'>
                    <div className='h-full bg-orange-500' style={{ width: '45%' }} />
                  </div>
                </div>
              </div>
            </div>
            <div>
              <h4 className='text-sm font-semibold mb-4'>Request Volume</h4>
              <div className='space-y-3'>
                <div className='flex items-center justify-between'>
                  <span className='text-sm text-muted-foreground'>Total Requests</span>
                  <span className='text-lg font-bold'>24,587</span>
                </div>
                <Separator />
                <div className='flex items-center justify-between'>
                  <span className='text-sm text-muted-foreground'>Success Rate</span>
                  <span className='text-lg font-bold text-green-600'>99.8%</span>
                </div>
                <Separator />
                <div className='flex items-center justify-between'>
                  <span className='text-sm text-muted-foreground'>Error Rate</span>
                  <span className='text-lg font-bold text-red-600'>0.2%</span>
                </div>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Recommendations */}
      <Card>
        <CardHeader>
          <CardTitle className='flex items-center gap-2'>
            <Zap className='h-5 w-5' />
            Recommendations
          </CardTitle>
          <CardDescription>AI-powered insights to optimize your deployment</CardDescription>
        </CardHeader>
        <CardContent>
          <div className='space-y-4'>
            <div className='flex gap-3 p-4 border rounded-lg bg-blue-50/50'>
              <div className='p-2 bg-blue-100 rounded-full h-fit'>
                <TrendingUp className='h-4 w-4 text-blue-600' />
              </div>
              <div className='flex-1'>
                <h4 className='text-sm font-semibold mb-1'>Scale Up Recommendation</h4>
                <p className='text-sm text-muted-foreground mb-2'>
                  Your deployment is consistently using 68% memory. Consider upgrading to the Premium plan for better performance.
                </p>
                <Button variant='outline' size='sm'>View Details</Button>
              </div>
            </div>
            <div className='flex gap-3 p-4 border rounded-lg bg-green-50/50'>
              <div className='p-2 bg-green-100 rounded-full h-fit'>
                <Cpu className='h-4 w-4 text-green-600' />
              </div>
              <div className='flex-1'>
                <h4 className='text-sm font-semibold mb-1'>Efficient Resource Usage</h4>
                <p className='text-sm text-muted-foreground mb-2'>
                  Your CPU usage is optimal. Current configuration is well-suited for your workload.
                </p>
              </div>
            </div>
            <div className='flex gap-3 p-4 border rounded-lg bg-yellow-50/50'>
              <div className='p-2 bg-yellow-100 rounded-full h-fit'>
                <AlertCircle className='h-4 w-4 text-yellow-600' />
              </div>
              <div className='flex-1'>
                <h4 className='text-sm font-semibold mb-1'>Update Available</h4>
                <p className='text-sm text-muted-foreground mb-2'>
                  Keycloak version 24.0.0 is available with security patches and performance improvements.
                </p>
                <Button variant='outline' size='sm'>Update Now</Button>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Usage Trends */}
      <Card>
        <CardHeader>
          <CardTitle className='flex items-center gap-2'>
            <BarChart3 className='h-5 w-5' />
            Usage Trends
          </CardTitle>
          <CardDescription>7-day historical data</CardDescription>
        </CardHeader>
        <CardContent>
          <div className='grid gap-4 md:grid-cols-3'>
            <div className='space-y-2'>
              <div className='flex items-center justify-between'>
                <span className='text-sm text-muted-foreground'>Avg CPU</span>
                <span className='text-sm font-bold'>38%</span>
              </div>
              <div className='text-xs text-green-600 flex items-center gap-1'>
                <TrendingUp className='h-3 w-3' />
                <span>+5% from last week</span>
              </div>
            </div>
            <div className='space-y-2'>
              <div className='flex items-center justify-between'>
                <span className='text-sm text-muted-foreground'>Avg Memory</span>
                <span className='text-sm font-bold'>65%</span>
              </div>
              <div className='text-xs text-green-600 flex items-center gap-1'>
                <TrendingUp className='h-3 w-3' />
                <span>+12% from last week</span>
              </div>
            </div>
            <div className='space-y-2'>
              <div className='flex items-center justify-between'>
                <span className='text-sm text-muted-foreground'>Avg Requests/min</span>
                <span className='text-sm font-bold'>1,247</span>
              </div>
              <div className='text-xs text-blue-600 flex items-center gap-1'>
                <TrendingUp className='h-3 w-3' />
                <span>-3% from last week</span>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
