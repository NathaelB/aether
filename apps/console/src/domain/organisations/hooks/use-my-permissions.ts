import { useQuery } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'
import { useResolvedOrganisationId } from './use-resolved-organisation-id'
import { can as holdsBit } from '../permissions'

/**
 * What the signed-in person may do in the organisation they are looking at.
 *
 * Read once and shared through the query cache, because nearly every screen
 * asks. `can` answers false while it is still loading: drawing a button and
 * taking it away is worse than drawing it a moment late, and the platform
 * refuses either way.
 */
export function useMyPermissions() {
  const organisationId = useResolvedOrganisationId()
  const accessToken = useAuthStore(selectAccessToken)

  const query = useQuery({
    ...window.api.get('/organisations/{organisation_id}/permissions', {
      path: { organisation_id: organisationId ?? 'current' },
    }).queryOptions,
    enabled: !!organisationId && !!accessToken,
  })

  const mask = query.data?.data?.permissions

  return {
    can: (bit: number) => holdsBit(mask, bit),
    isLoading: query.isLoading,
  }
}
