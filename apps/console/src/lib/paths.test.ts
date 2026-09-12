import { describe, expect, it } from 'vitest'
import { belongsToNoOrganisation, isUnder } from './paths'

const DEPLOYMENT = '/organisations/org-1/deployments/dep-1'

describe('isUnder', () => {
  it('lights up on its own page', () => {
    expect(isUnder(DEPLOYMENT, DEPLOYMENT)).toBe(true)
  })

  it('lights up on a page beneath it', () => {
    expect(isUnder(`${DEPLOYMENT}/logs`, DEPLOYMENT)).toBe(true)
  })

  /**
   * The reason this is a function rather than a `startsWith` at the call
   * site: a prefix that stops mid segment points at a different page.
   */
  it('does not light up on a path that merely starts the same way', () => {
    expect(isUnder('/organisations/org-1/deployments-archive', '/organisations/org-1/deployments')).toBe(false)
  })

  /**
   * The overview of a deployment is the deployment's own path, so without
   * this it would stay lit on every page under it.
   */
  it('an exact entry only lights up on itself', () => {
    expect(isUnder(DEPLOYMENT, DEPLOYMENT, true)).toBe(true)
    expect(isUnder(`${DEPLOYMENT}/logs`, DEPLOYMENT, true)).toBe(false)
  })

  it('ignores a trailing slash on either side', () => {
    expect(isUnder(`${DEPLOYMENT}/`, DEPLOYMENT, true)).toBe(true)
    expect(isUnder(DEPLOYMENT, `${DEPLOYMENT}/`, true)).toBe(true)
  })

  it('does not light up on a sibling', () => {
    expect(isUnder(`${DEPLOYMENT}/usage`, `${DEPLOYMENT}/logs`)).toBe(false)
  })
})

describe('belongsToNoOrganisation', () => {
  /**
   * The bug this exists to close: the console sends anyone with no
   * organisation in the URL back to their own, so reaching the platform
   * bounced in the same frame and the link looked like a dead button.
   */
  it('knows the platform stands outside any organisation', () => {
    expect(belongsToNoOrganisation('/platform')).toBe(true)
    expect(belongsToNoOrganisation('/platform/dataplanes')).toBe(true)
    expect(belongsToNoOrganisation('/platform/releases')).toBe(true)
  })

  it('knows creating an organisation happens before there is one', () => {
    expect(belongsToNoOrganisation('/organisations/create')).toBe(true)
  })

  /**
   * Everything else does belong to one, including an organisation whose id
   * happens to start the same way as the create route.
   */
  it('sends everything else back to an organisation', () => {
    expect(belongsToNoOrganisation('/organisations/org-1')).toBe(false)
    expect(belongsToNoOrganisation('/organisations/org-1/deployments')).toBe(false)
    expect(belongsToNoOrganisation('/organisations/created-org')).toBe(false)
    expect(belongsToNoOrganisation('/')).toBe(false)
  })
})

describe('an invitation link', () => {
  /**
   * The page that puts somebody in an organisation cannot be behind the
   * redirect that sends them to one they are already in. Somebody with no
   * organisation would be asked to create one instead of joining the one
   * they were invited to; somebody with one would never reach the link.
   */
  it('escapes the redirect that sends people to an organisation', () => {
    expect(belongsToNoOrganisation('/invitations/accept')).toBe(true)
  })

  it('does not swallow anything else beginning the same way', () => {
    expect(belongsToNoOrganisation('/invitations')).toBe(false)
    expect(belongsToNoOrganisation('/invitations/accepted-elsewhere')).toBe(false)
  })
})
