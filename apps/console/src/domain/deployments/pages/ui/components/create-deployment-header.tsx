import { Button } from '@/components/ui/button'
import { ArrowLeft } from 'lucide-react'
import { useNavigate } from '@tanstack/react-router'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'

export function CreateDeploymentHeader() {
  const navigate = useNavigate()
  const organisationPath = useOrganisationPath()
  
  return (
    <div className='flex items-center gap-4 pb-6 border-b mb-6'>
      <Button
        variant='ghost'
        size='icon'
        onClick={() => navigate({ to: organisationPath('/deployments') })}
      >
        <ArrowLeft className='h-5 w-5' />
      </Button>
      <div>
        <h1 className='text-xl font-semibold tracking-tight'>Create Deployment</h1>
        <p className='text-sm text-muted-foreground'>Configure and deploy a new IAM service</p>
      </div>
    </div>
  )
}
