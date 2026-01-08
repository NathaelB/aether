import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Info } from 'lucide-react'
import { DeploymentPlan, DEPLOYMENT_PLANS } from '../../../types/deployment'

interface Props {
  plan: DeploymentPlan;
  capacity: number;
}

export function DeploymentCostEstimator({ plan, capacity }: Props) {
  const selectedPlanInfo = DEPLOYMENT_PLANS[plan]
  
  // Mock cost calculation: Base Price + (Capacity / 100) * Multiplier
  const estimatedCost = plan === 'freemium' ? 0 : selectedPlanInfo.basePrice + (capacity / 100) * 2

  return (
    <div className='hidden lg:block w-80 space-y-6'>
      <Card className='bg-muted/50 border-none shadow-none'>
        <CardHeader>
          <CardTitle className='text-base'>Monthly Estimate</CardTitle>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='flex justify-between items-center text-sm'>
            <span>Base Plan ({selectedPlanInfo.label})</span>
            <span className='font-medium'>
              {selectedPlanInfo.basePrice === 0 ? 'Free' : `$${selectedPlanInfo.basePrice}.00`}
            </span>
          </div>
          <div className='flex justify-between items-center text-sm'>
            <span>User Capacity ({capacity})</span>
            <span className='font-medium'>
              {plan === 'freemium' ? 'Free' : `$${((capacity / 100) * 2).toFixed(2)}`}
            </span>
          </div>
          <div className='border-t pt-4 flex justify-between items-center font-bold'>
            <span>Total</span>
            <span>
              ${estimatedCost.toFixed(2)} / mo
            </span>
          </div>
        </CardContent>
      </Card>

      <div className='p-4 rounded-lg border bg-background space-y-2'>
        <div className='flex items-center gap-2 font-medium text-sm'>
          <Info className='h-4 w-4 text-primary' />
          Plan Limits
        </div>
        <div className='text-xs text-muted-foreground leading-relaxed space-y-1'>
          <div className='flex justify-between'>
            <span>Realms:</span>
            <span className='font-medium text-foreground'>{selectedPlanInfo.maxRealms === 1 ? '1 (Strict Limit)' : `Up to ${selectedPlanInfo.maxRealms}`}</span>
          </div>
          <div className='flex justify-between'>
            <span>Support:</span>
            <span className='font-medium text-foreground'>{plan === 'freemium' ? 'Community' : 'Standard'}</span>
          </div>
        </div>
        <Button variant='link' className='text-xs h-auto p-0'>
          View Documentation
        </Button>
      </div>
    </div>
  )
}
