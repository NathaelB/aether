import { describe, expect, it } from 'vitest'
import { organisationIdIn, routeToAnOrganisation, type OrganisationRouting } from './routing'

const ORGANISATIONS = [{ id: 'org-1' }, { id: 'org-2' }]

function state(over: Partial<OrganisationRouting> = {}): OrganisationRouting {
  return {
    pathname: '/',
    organisations: ORGANISATIONS,
    activeOrganisationId: null,
    loaded: true,
    loadSucceeded: true,
    ...over,
  }
}

describe('routeToAnOrganisation', () => {
  /**
   * The bug this was extracted for. The platform console has no organisation
   * by design, and sending it home bounced in the same frame, so the link
   * that led there looked like a dead button.
   */
  it('leaves the platform alone', () => {
    expect(routeToAnOrganisation(state({ pathname: '/platform/dataplanes' }))).toEqual({})
    expect(routeToAnOrganisation(state({ pathname: '/platform' }))).toEqual({})
  })

  /** Including for somebody who has no organisation at all to be sent to. */
  it('leaves the platform alone even with nothing to fall back to', () => {
    const decision = routeToAnOrganisation(
      state({ pathname: '/platform/releases', organisations: [] }),
    )

    expect(decision).toEqual({})
  })

  it('leaves the create form alone', () => {
    expect(routeToAnOrganisation(state({ pathname: '/organisations/create' }))).toEqual({})
  })

  it('sends somebody at the root to an organisation', () => {
    expect(routeToAnOrganisation(state()).navigateTo).toBe('/organisations/org-1')
  })

  /** A link somebody pasted has to open what it opened for them. */
  it('follows the URL over what was last active', () => {
    const decision = routeToAnOrganisation(
      state({ pathname: '/organisations/org-2/deployments', activeOrganisationId: 'org-1' }),
    )

    expect(decision).toEqual({ setActive: 'org-2' })
  })

  it('does nothing when the URL already names what is active', () => {
    const decision = routeToAnOrganisation(
      state({ pathname: '/organisations/org-1', activeOrganisationId: 'org-1' }),
    )

    expect(decision).toEqual({})
  })

  /** An organisation somebody was removed from, or one that was deleted. */
  it('falls back when the URL names an organisation that is not theirs', () => {
    const decision = routeToAnOrganisation(
      state({ pathname: '/organisations/gone', activeOrganisationId: 'org-2' }),
    )

    expect(decision).toEqual({ navigateTo: '/organisations/org-2' })
  })

  it('offers the create form to somebody with none', () => {
    const decision = routeToAnOrganisation(state({ organisations: [] }))

    expect(decision.navigateTo).toBe('/organisations/create')
  })

  /**
   * Only once the list is known to be empty. Pushing the form at somebody
   * whose organisations failed to load invites them to make a second one
   * they already have.
   */
  it('waits rather than offering the form on a failed read', () => {
    expect(routeToAnOrganisation(state({ organisations: [], loadSucceeded: false }))).toEqual({})
    expect(routeToAnOrganisation(state({ loaded: false }))).toEqual({})
  })
})

describe('organisationIdIn', () => {
  it('reads the organisation out of a path', () => {
    expect(organisationIdIn('/organisations/org-1')).toBe('org-1')
    expect(organisationIdIn('/organisations/org-1/deployments/dep-1')).toBe('org-1')
  })

  /** The form for making one, not the id of one. */
  it('does not read the create form as an organisation', () => {
    expect(organisationIdIn('/organisations/create')).toBeNull()
  })

  it('finds nothing where there is nothing', () => {
    expect(organisationIdIn('/platform/dataplanes')).toBeNull()
    expect(organisationIdIn('/')).toBeNull()
  })
})
