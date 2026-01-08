import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import { Box, Users } from 'lucide-react'
import { DeploymentPlan, DEPLOYMENT_PLANS, DEPLOYMENT_CAPACITIES } from '../../../types/deployment'

interface Props {
  plan: DeploymentPlan;
  setPlan: (plan: DeploymentPlan) => void;
  capacity: number;
  handleCapacityChange: (capacity: number) => void;
}

export function DeploymentPlanSelector({ 
  plan, 
  setPlan, 
  capacity, 
  handleCapacityChange 
}: Props) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className='text-lg flex items-center gap-2'>
          <Box className='h-5 w-5 text-muted-foreground' />
          Plan & Capacity
        </CardTitle>
        <CardDescription>Choose your user capacity and resource plan.</CardDescription>
      </CardHeader>
      <CardContent className='space-y-6'>
        
        {/* Capacity Selector */}
        <div className='space-y-3'>
          <Label className='flex items-center gap-2'>
            <Users className='h-4 w-4' />
            Expected Active Users
          </Label>
          <div className='grid grid-cols-4 md:grid-cols-7 gap-2'>
            {DEPLOYMENT_CAPACITIES.map((cap) => (
              <div 
                key={cap}
                className={`cursor-pointer rounded-md border p-2 text-center text-sm hover:bg-accent transition-colors ${capacity === cap ? 'border-primary ring-1 ring-primary bg-accent/50 font-medium' : ''}`}
                onClick={() => handleCapacityChange(cap)}
              >
                {cap >= 1000 ? `${cap/1000}k` : cap}
              </div>
            ))}
          </div>
          {capacity === 100 && (
            <p className='text-xs text-amber-600 font-medium'>
              100 users capacity is limited to the Freemium plan.
            </p>
          )}
        </div>

        {/* Plan Selector */}
        <div className='space-y-3'>
          <Label>Deployment Plan</Label>
          <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
            {(Object.entries(DEPLOYMENT_PLANS) as [DeploymentPlan, typeof DEPLOYMENT_PLANS[DeploymentPlan]][]).map(([key, info]) => {
              const isDisabled = (capacity === 100 && key !== 'freemium') || (capacity > 100 && key === 'freemium')
              
              if (isDisabled) return null

              return (
                <div 
                  key={key}
                  className={`cursor-pointer rounded-lg border p-4 transition-all ${plan === key ? 'border-primary ring-1 ring-primary bg-accent/50' : 'hover:bg-accent'}`}
                  onClick={() => setPlan(key)}
                >
                  <div className='flex justify-between items-start'>
                    <div className='font-semibold'>{info.label}</div>
                    <div className='text-sm font-medium'>
                      {info.basePrice === 0 ? 'Free' : `$${info.basePrice}/mo base`}
                    </div>
                  </div>
                  <div className='text-xs text-muted-foreground mt-1 mb-3'>{info.description}</div>
                  <div className='grid grid-cols-2 gap-2 text-xs font-mono text-muted-foreground'>
                    <div className='bg-background/50 p-1 rounded px-2 border'>{info.cpu}</div>
                    <div className='bg-background/50 p-1 rounded px-2 border'>{info.memory}</div>
                  </div>
                </div>
              )
            })}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
