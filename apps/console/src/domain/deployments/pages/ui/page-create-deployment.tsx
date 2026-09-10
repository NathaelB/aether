import { Button } from '@/components/ui/button'
import { useState } from 'react'
import { DeploymentType, Environment, DeploymentPlan, DeploymentMode } from '../../types/deployment'
import { useNavigate } from '@tanstack/react-router'
import { CreateDeploymentHeader } from './components/create-deployment-header'
import { DeploymentIdentityProviderSelector } from './components/deployment-identity-provider-selector'
import { DeploymentConfigurationForm } from './components/deployment-configuration-form'
import { DeploymentModeSelector } from './components/deployment-mode-selector'
import { DeploymentPlanSelector } from './components/deployment-plan-selector'
import { DeploymentCostEstimator } from './components/deployment-cost-estimator'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'

interface PageCreateDeploymentProps {
  onSubmit: (data: {
    name: string;
    type: DeploymentType;
    environment: Environment;
    region: string;
    mode: DeploymentMode;
    plan: DeploymentPlan;
    capacity: number;
  }) => void
  isSubmitting?: boolean
  /** Regions an existing data plane serves. See `servedRegions`. */
  regions: string[]
  regionsLoading: boolean
}

export default function PageCreateDeployment({
  onSubmit,
  isSubmitting = false,
  regions,
  regionsLoading,
}: PageCreateDeploymentProps) {
  const navigate = useNavigate()
  const organisationPath = useOrganisationPath()
  const [name, setName] = useState('')
  const [type, setType] = useState<DeploymentType>('keycloak')
  const [environment, setEnvironment] = useState<Environment>('development')
  const [mode, setMode] = useState<DeploymentMode>('shared')
  const [plan, setPlan] = useState<DeploymentPlan>('starter')

  // The regions arrive from the API, so the first one cannot be an initial
  // state -- but syncing it in an effect would re-render for a value that is
  // already knowable. Only the user's choice is state; the effective region is
  // derived, and falls back to the first served one until they pick.
  const [chosenRegion, setChosenRegion] = useState<string | null>(null)
  const region = chosenRegion ?? regions[0] ?? ''
  const setRegion = setChosenRegion
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
    onSubmit({ name, type, environment, region, mode, plan, capacity })
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
            regions={regions}
            regionsLoading={regionsLoading}
          />

          <DeploymentModeSelector mode={mode} onSelect={setMode} />

          <DeploymentPlanSelector 
            plan={plan}
            setPlan={setPlan}
            capacity={capacity}
            handleCapacityChange={handleCapacityChange}
          />

          {/* Actions */}
          <div className='flex items-center justify-between pt-4'>
              <Button
                variant='ghost'
                type='button'
                onClick={() => navigate({ to: organisationPath('/deployments') })}
              >
                Cancel
              </Button>
              <Button type='submit' disabled={isSubmitting || region === ''} size='lg'>
                {isSubmitting ? 'Creating...' : 'Create Deployment'}
              </Button>
          </div>
        </form>

        <DeploymentCostEstimator plan={plan} capacity={capacity} />
      </div>
    </div>
  )
}
