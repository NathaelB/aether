import { useParams } from '@tanstack/react-router'
import { useGetDeployment } from '@/api/deployment.api'
import { PageDeploymentResources } from '../ui/page-deployment-resources'

export default function PageDeploymentResourcesFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const deployment = useGetDeployment(deploymentId ?? null)

  return (
    <PageDeploymentResources
      deployment={deployment.data?.data}
      isLoading={deployment.isLoading}
    />
  )
}
