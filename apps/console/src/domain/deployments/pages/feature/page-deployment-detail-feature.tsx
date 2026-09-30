import { useParams } from '@tanstack/react-router'
import { useGetDeployment, useGetDeploymentActions } from '@/api/deployment.api'
import { useGetDeploymentUptime } from '@/api/deployment-uptime.api'
import { useHoldsPlatformRight } from '@/domain/organisations/hooks/use-is-operator'
import { availabilityView } from '../../availability'
import { PageDeploymentDetail } from '../ui/page-deployment-detail'

export default function PageDeploymentDetailFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }

  const deployment = useGetDeployment(deploymentId ?? null)
  const actions = useGetDeploymentActions(deploymentId ?? null)

  const canViewEstate = useHoldsPlatformRight('view_estate')
  const uptime = useGetDeploymentUptime(deploymentId ?? null, canViewEstate)
  const availability = availabilityView(canViewEstate, uptime)

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
      availability={availability}
      isLoading={deployment.isLoading}
    />
  )
}
