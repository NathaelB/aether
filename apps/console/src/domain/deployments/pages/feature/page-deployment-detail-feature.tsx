import { useParams } from '@tanstack/react-router'
import { useGetDeployment, useGetDeploymentActions } from '@/api/deployment.api'
import { PageDeploymentDetail } from '../ui/page-deployment-detail'

export default function PageDeploymentDetailFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }

  const deployment = useGetDeployment(deploymentId ?? null)
  const actions = useGetDeploymentActions(deploymentId ?? null)

  const filteredActions = actions.data?.data
    .filter((action) => {
      if (action.action_type.startsWith('system.')) {
        return false
      }
      const source = action.metadata.source
      if (typeof source === 'string' && source === 'System') {
        return false
      }
      if (typeof source === 'object' && 'System' in source) {
        return false
      }
      return true
    })
    .sort((a, b) => {
      const dateA = new Date(a.metadata.created_at).getTime()
      const dateB = new Date(b.metadata.created_at).getTime()
      return dateB - dateA
    }) ?? []

  return (
    <PageDeploymentDetail
      deployment={deployment.data?.data}
      actions={filteredActions}
      isLoading={deployment.isLoading}
    />
  )
}
