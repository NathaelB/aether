import { useEffect } from 'react'
import { useNavigate, useRouterState } from '@tanstack/react-router'
import { useUserOrganisations } from '../../hooks/use-user-organisations'
import {
  selectOrganisations,
  selectActiveOrganisationId,
  selectOrganisationsLoaded,
  useOrganisationsStore,
} from '@/stores/organisations'
import { useOrganisationIdFromUrl } from '../../hooks/use-organisation-id-from-url'

export function OrganisationsBootstrap() {
  const navigate = useNavigate()
  const { location } = useRouterState()
  const organisations = useOrganisationsStore(selectOrganisations)
  const organisationsLoaded = useOrganisationsStore(selectOrganisationsLoaded)
  const activeOrganisationId = useOrganisationsStore(selectActiveOrganisationId)
  const setActiveOrganisationId = useOrganisationsStore(
    (state) => state.setActiveOrganisationId
  )
  const urlOrganisationId = useOrganisationIdFromUrl()
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

    if (!organisationsLoaded || organisations.length === 0 || isCreateOrganisationRoute) {
      return
    }

    const urlOrganisationExists =
      !!urlOrganisationId && organisations.some((org) => org.id === urlOrganisationId)

    if (urlOrganisationExists) {
      if (activeOrganisationId !== urlOrganisationId) {
        setActiveOrganisationId(urlOrganisationId)
      }
      return
    }

    const storedOrganisationExists =
      !!activeOrganisationId && organisations.some((org) => org.id === activeOrganisationId)

    const fallbackOrganisationId = storedOrganisationExists
      ? activeOrganisationId
      : organisations[0].id

    if (activeOrganisationId !== fallbackOrganisationId) {
      setActiveOrganisationId(fallbackOrganisationId)
    }

    navigate({
      to: `/organisations/${fallbackOrganisationId}`,
      replace: true,
    })
  }, [
    isSuccess,
    organisationsLoaded,
    organisations.length,
    isCreateOrganisationRoute,
    urlOrganisationId,
    activeOrganisationId,
    navigate,
    setActiveOrganisationId,
    organisations,
  ])

  return null
}
