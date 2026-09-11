import { useParams } from '@tanstack/react-router'
import { useGetDeployment } from '@/api/deployment.api'
import { PageDeploymentGeneral } from '../ui/page-deployment-general'

export default function PageDeploymentGeneralFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const deployment = useGetDeployment(deploymentId ?? null)

  return (
    <PageDeploymentGeneral
      deployment={deployment.data?.data}
      isLoading={deployment.isLoading}
    />
  )
}
