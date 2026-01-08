import { Button } from '@/components/ui/button'
import { useState } from 'react'
import { DeploymentType, Environment, DeploymentPlan } from '../../types/deployment'
import { useNavigate } from '@tanstack/react-router'
import { CreateDeploymentHeader } from './components/create-deployment-header'
import { DeploymentIdentityProviderSelector } from './components/deployment-identity-provider-selector'
import { DeploymentConfigurationForm } from './components/deployment-configuration-form'
import { DeploymentPlanSelector } from './components/deployment-plan-selector'
import { DeploymentCostEstimator } from './components/deployment-cost-estimator'

interface PageCreateDeploymentProps {
  onSubmit: (data: { 
    name: string; 
    type: DeploymentType; 
    environment: Environment; 
    region: string;
    plan: DeploymentPlan;
    capacity: number;
  }) => void
  isSubmitting?: boolean
}

export default function PageCreateDeployment({ onSubmit, isSubmitting = false }: PageCreateDeploymentProps) {
  const navigate = useNavigate()
  const [name, setName] = useState('')
  const [type, setType] = useState<DeploymentType>('keycloak')
  const [environment, setEnvironment] = useState<Environment>('development')
  const [region, setRegion] = useState('us-east-1')
  const [plan, setPlan] = useState<DeploymentPlan>('starter')
  const [capacity, setCapacity] = useState<number>(250)

  const handleCapacityChange = (newCapacity: number) => {
    setCapacity(newCapacity)
    if (newCapacity === 100) {
        setPlan('freemium')
    } else if (plan === 'freemium') {
        setPlan('starter')
    }
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit({ name, type, environment, region, plan, capacity })
  }

  return (
    <div className='flex flex-col h-full'>
      <CreateDeploymentHeader />

      <div className='flex gap-8 items-start'>
        {/* Main Form Area */}
        <form onSubmit={handleSubmit} className='flex-1 space-y-6 max-w-3xl'>
          
          <DeploymentIdentityProviderSelector 
            selectedType={type} 
            onSelect={setType} 
          />

          <DeploymentConfigurationForm 
            name={name}
            setName={setName}
            environment={environment}
            setEnvironment={setEnvironment}
            region={region}
            setRegion={setRegion}
          />

          <DeploymentPlanSelector 
            plan={plan}
            setPlan={setPlan}
            capacity={capacity}
            handleCapacityChange={handleCapacityChange}
          />

          {/* Actions */}
          <div className='flex items-center justify-between pt-4'>
              <Button variant='ghost' type='button' onClick={() => navigate({ to: '/deployments' })}>
                Cancel
              </Button>
              <Button type='submit' disabled={isSubmitting} size='lg'>
                {isSubmitting ? 'Creating...' : 'Create Deployment'}
              </Button>
          </div>
        </form>

        <DeploymentCostEstimator plan={plan} capacity={capacity} />
      </div>
    </div>
  )
}