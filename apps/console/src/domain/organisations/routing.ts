import { belongsToNoOrganisation, organisationPathFor } from '@/lib/paths'

export interface OrganisationRouting {
  pathname: string
  organisations: { id: string }[]
  activeOrganisationId: string | null
  /** Whether the list has been read back, however it turned out. */
  loaded: boolean
  /** Whether reading it succeeded, as opposed to failing or still running. */
  loadSucceeded: boolean
}

/** What the console should do about the organisation, if anything. */
export interface OrganisationDecision {
  setActive?: string
  navigateTo?: string
}

const NOTHING: OrganisationDecision = {}

/**
 * Where somebody belongs, given where they are.
 *
 * A function rather than a chain of conditions inside an effect, because the
 * effect runs on every route change and reads five pieces of state: the two
 * times it has been wrong, the wrongness was in this decision and there was
 * no way to look at it without a browser.
 */
export function routeToAnOrganisation(state: OrganisationRouting): OrganisationDecision {
  // Somewhere that has no organisation on purpose. Sending them home is how
  // the link that led here looks like a button that does nothing.
  if (belongsToNoOrganisation(state.pathname)) return NOTHING

  if (!state.loaded) return NOTHING

  if (state.organisations.length === 0) {
    // Only once the list is known to be empty rather than merely unread:
    // pushing the create form at somebody whose organisations failed to load
    // invites them to make a second one they already have.
    return state.loadSucceeded ? { navigateTo: '/organisations/create' } : NOTHING
  }

  const fromUrl = organisationIdIn(state.pathname)
  const urlIsReal = !!fromUrl && state.organisations.some((one) => one.id === fromUrl)

  // The URL wins over what was last active: a link somebody pasted has to
  // open what it opened for them.
  if (urlIsReal) {
    return fromUrl === state.activeOrganisationId ? NOTHING : { setActive: fromUrl }
  }

  const activeIsReal =
    !!state.activeOrganisationId &&
    state.organisations.some((one) => one.id === state.activeOrganisationId)

  const fallback = activeIsReal ? state.activeOrganisationId! : state.organisations[0].id

  return {
    ...(fallback === state.activeOrganisationId ? {} : { setActive: fallback }),
    navigateTo: organisationPathFor(fallback),
  }
}

/**
 * The organisation named in a path, if it names one.
 *
 * `create` is the form for making one, not the id of one.
 */
export function organisationIdIn(pathname: string): string | null {
  const match = pathname.match(/^\/organisations\/([^/]+)(?:\/|$)/)
  if (!match) return null

  const candidate = decodeURIComponent(match[1])
  return candidate === 'create' ? null : candidate
}
