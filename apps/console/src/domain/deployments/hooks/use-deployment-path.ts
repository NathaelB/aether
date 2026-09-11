import { useCallback } from 'react'
import { useParams } from '@tanstack/react-router'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'

/**
 * Paths inside the deployment that is currently open.
 *
 * A hook rather than a helper taking the id, because every caller is already
 * inside the deployment's own layout and passing the id back down would be
 * threading a value the router already holds.
 */
export function useDeploymentPath() {
  const organisationPath = useOrganisationPath()
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }

  return useCallback(
    (path = '') => organisationPath(`/deployments/${deploymentId ?? ''}${path}`),
    [organisationPath, deploymentId],
  )
}
