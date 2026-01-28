import { useCallback } from 'react'
import { useResolvedOrganisationId } from './use-resolved-organisation-id'

export const useOrganisationPath = () => {
  const organisationId = useResolvedOrganisationId()

  return useCallback(
    (path = '') => {
      if (!organisationId) {
        return path || '/'
      }

      const suffix = path ? (path.startsWith('/') ? path : `/${path}`) : ''
      return `/organisations/${organisationId}${suffix}`
    },
    [organisationId]
  )
}
