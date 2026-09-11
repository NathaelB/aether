import { useEffect } from 'react'
import { useNavigate, useRouterState } from '@tanstack/react-router'
import { useUserOrganisations } from '../../hooks/use-user-organisations'
import { routeToAnOrganisation } from '../../routing'
import {
  selectActiveOrganisationId,
  selectOrganisations,
  selectOrganisationsLoaded,
  useOrganisationsStore,
} from '@/stores/organisations'

/**
 * Keeps the URL and the active organisation agreeing with each other.
 *
 * Renders nothing: it sits above the router so that arriving anywhere ends
 * up somewhere that exists. What it decides lives in `routeToAnOrganisation`,
 * where it can be read without a browser.
 */
export function OrganisationsBootstrap() {
  const navigate = useNavigate()
  const { location } = useRouterState()
  const organisations = useOrganisationsStore(selectOrganisations)
  const organisationsLoaded = useOrganisationsStore(selectOrganisationsLoaded)
  const activeOrganisationId = useOrganisationsStore(selectActiveOrganisationId)
  const setActiveOrganisationId = useOrganisationsStore((state) => state.setActiveOrganisationId)
  const { isSuccess } = useUserOrganisations()

  const { setActive, navigateTo } = routeToAnOrganisation({
    pathname: location.pathname,
    organisations,
    activeOrganisationId,
    loaded: organisationsLoaded,
    loadSucceeded: isSuccess,
  })

  useEffect(() => {
    if (setActive) setActiveOrganisationId(setActive)
    if (navigateTo) navigate({ to: navigateTo, replace: true })
  }, [setActive, navigateTo, setActiveOrganisationId, navigate])

  return null
}
