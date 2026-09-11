import { useMemo } from 'react'
import { useRouterState } from '@tanstack/react-router'
import { organisationIdIn } from '../routing'

export const useOrganisationIdFromUrl = () => {
  const { location } = useRouterState()

  return useMemo(() => organisationIdIn(location.pathname), [location.pathname])
}
