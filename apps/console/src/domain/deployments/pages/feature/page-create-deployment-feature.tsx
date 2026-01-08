import { useNavigate } from '@tanstack/react-router'
import PageCreateDeployment from '../ui/page-create-deployment'
import { useState } from 'react'
import { DeploymentType, Environment, DeploymentPlan } from '../../types/deployment'

export default function PageCreateDeploymentFeature() {
  const navigate = useNavigate()
  const [isSubmitting, setIsSubmitting] = useState(false)

  const handleCreate = async (data: { 
    name: string; 
    type: DeploymentType; 
    environment: Environment;
    region: string;
    plan: DeploymentPlan;
    capacity: number;
  }) => {
    setIsSubmitting(true)
    // Mock request
    console.log('Creating deployment with data:', data)
    
    // Simulate API delay
    await new Promise(resolve => setTimeout(resolve, 1500))
    
    // In a real app, we would use a mutation here.
    // Since we are mocking, we just log and navigate.
    
    setIsSubmitting(false)
    navigate({ to: '/deployments' })
  }

  return (
    <PageCreateDeployment onSubmit={handleCreate} isSubmitting={isSubmitting} />
  )
}