import { useOrganisationIdFromUrl } from './use-organisation-id-from-url'
import { selectActiveOrganisationId, useOrganisationsStore } from '@/stores/organisations'

export const useResolvedOrganisationId = () => {
  const urlOrganisationId = useOrganisationIdFromUrl()
  const activeOrganisationId = useOrganisationsStore(selectActiveOrganisationId)

  return urlOrganisationId ?? activeOrganisationId
}
