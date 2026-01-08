import { useParams, useNavigate } from '@tanstack/react-router'
import { useDeployment } from '../../hooks/use-deployment'
import { PageDeploymentDetail } from '../ui/page-deployment-detail'
import { Card, CardContent } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { Button } from '@/components/ui/button'
import { ArrowLeft } from 'lucide-react'

export default function PageDeploymentDetailFeature() {
  const { deploymentId } = useParams({ strict: false })
  const navigate = useNavigate()
  const { data: deployment, isLoading, error, refetch } = useDeployment(deploymentId as string)

  if (isLoading) {
    return (
      <div className='space-y-6'>
        <div className='flex items-center gap-4'>
          <Skeleton className='h-10 w-10' />
          <Skeleton className='h-8 w-64' />
        </div>
        <div className='grid gap-4 md:grid-cols-3'>
          {[...Array(3)].map((_, i) => (
            <Card key={i}>
              <CardContent className='p-6'>
                <Skeleton className='h-4 w-24 mb-2' />
                <Skeleton className='h-6 w-32' />
              </CardContent>
            </Card>
          ))}
        </div>
        <Card>
          <CardContent className='p-6'>
            <div className='space-y-4'>
              {[...Array(6)].map((_, i) => (
                <div key={i} className='flex items-center justify-between'>
                  <Skeleton className='h-4 w-32' />
                  <Skeleton className='h-4 w-48' />
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>
    )
  }

  if (error) {
    return (
      <div className='flex h-[50vh] w-full items-center justify-center'>
        <div className='text-center space-y-4'>
          <p className='text-lg font-semibold text-red-600'>Error loading deployment</p>
          <p className='text-muted-foreground'>
            {error instanceof Error ? error.message : 'An unexpected error occurred'}
          </p>
          <Button onClick={() => navigate({ to: '/deployments' })}>
            Back to Deployments
          </Button>
        </div>
      </div>
    )
  }

  if (!deployment) {
    return (
      <div className='flex h-[50vh] w-full items-center justify-center'>
        <div className='text-center space-y-4'>
          <p className='text-lg font-semibold'>Deployment not found</p>
          <p className='text-muted-foreground'>
            The deployment you're looking for doesn't exist or has been deleted.
          </p>
          <Button onClick={() => navigate({ to: '/deployments' })}>
            <ArrowLeft className='mr-2 h-4 w-4' />
            Back to Deployments
          </Button>
        </div>
      </div>
    )
  }

  return (
    <PageDeploymentDetail
      deployment={deployment}
      onRefresh={() => refetch()}
      onBack={() => navigate({ to: '/deployments' })}
    />
  )
}
