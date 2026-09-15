import { useMutation, useQuery } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'
import type { EstateFilters } from '@/domain/platform/estate-filters'
import { toEstateQuery } from '@/domain/platform/estate-filters'

/**
 * Every deployment on the installation, narrowed server-side.
 *
 * The filters travel rather than being applied here: an estate of two hundred
 * deployments would be two hundred rows fetched to draw the four somebody
 * asked about, and the page after them would be missing.
 */
export const useGetEstateDeployments = (filters: EstateFilters) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/platform/deployments', { query: toEstateQuery(filters) }).queryOptions,
    enabled: !!accessToken,
  })
}

export const useGetTenant = (organisationId: string | null) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/platform/organisations/{organisation_id}', {
      path: { organisation_id: organisationId ?? 'current' },
    }).queryOptions,
    enabled: !!organisationId && !!accessToken,
  })
}

export const useGetTenants = () => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/platform/organisations', { query: {} }).queryOptions,
    enabled: !!accessToken,
  })
}

/**
 * Asking for an archive of somebody's deployment.
 *
 * Nothing is invalidated on success, deliberately. The archive does not exist
 * yet — the data plane has been told — and refetching a list that cannot have
 * changed would draw the same rows back and read as nothing having happened.
 */
export const useAskForBackup = () => {
  return useMutation({
    ...window.api.mutation(
      'post',
      '/organisations/{organisation_id}/deployments/{deployment_id}/backups',
    ).mutationOptions,
  })
}
