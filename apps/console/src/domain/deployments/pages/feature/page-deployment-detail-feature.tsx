import { useParams } from '@tanstack/react-router'
import { useGetDeployment, useGetDeploymentActions } from '@/api/deployment.api'
import { PageDeploymentDetail } from '../ui/page-deployment-detail'

export default function PageDeploymentDetailFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }

  const deployment = useGetDeployment(deploymentId ?? null)
  const actions = useGetDeploymentActions(deploymentId ?? null)

  return (
    <PageDeploymentDetail
      deployment={deployment.data?.data}
      actions={actions.data?.data ?? []}
      isLoading={deployment.isLoading}
    />
  )
}
