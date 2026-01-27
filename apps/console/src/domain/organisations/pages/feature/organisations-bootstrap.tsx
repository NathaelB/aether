import { useEffect } from 'react'
import { useNavigate, useRouterState } from '@tanstack/react-router'
import { useUserOrganisations } from '../../hooks/use-user-organisations'
import {
  selectOrganisations,
  selectOrganisationsLoaded,
  useOrganisationsStore,
} from '@/stores/organisations'

export function OrganisationsBootstrap() {
  const navigate = useNavigate()
  const { location } = useRouterState()
  const organisations = useOrganisationsStore(selectOrganisations)
  const organisationsLoaded = useOrganisationsStore(selectOrganisationsLoaded)
  const { isSuccess } = useUserOrganisations()

  const isCreateOrganisationRoute = location.pathname === '/organisations/create'

  useEffect(() => {
    if (
      isSuccess &&
      organisationsLoaded &&
      organisations.length === 0 &&
      !isCreateOrganisationRoute
    ) {
      navigate({ to: '/organisations/create', replace: true })
    }
  }, [
    isSuccess,
    organisationsLoaded,
    organisations.length,
    isCreateOrganisationRoute,
    navigate,
  ])

  return null
}
