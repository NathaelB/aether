import { describe, expect, it } from 'vitest'
import { isActive } from './nav-active'

const DEPLOYMENT = '/organisations/org-1/deployments/dep-1'

describe('isActive', () => {
  it('lights up on its own page', () => {
    expect(isActive(DEPLOYMENT, DEPLOYMENT)).toBe(true)
  })

  it('lights up on a page beneath it', () => {
    expect(isActive(`${DEPLOYMENT}/logs`, DEPLOYMENT)).toBe(true)
  })

  /**
   * The reason this is a function rather than a `startsWith` at the call
   * site: a prefix that stops mid segment points at a different page.
   */
  it('does not light up on a path that merely starts the same way', () => {
    expect(isActive('/organisations/org-1/deployments-archive', '/organisations/org-1/deployments')).toBe(false)
  })

  /**
   * The overview of a deployment is the deployment's own path, so without
   * this it would stay lit on every page under it.
   */
  it('an exact entry only lights up on itself', () => {
    expect(isActive(DEPLOYMENT, DEPLOYMENT, true)).toBe(true)
    expect(isActive(`${DEPLOYMENT}/logs`, DEPLOYMENT, true)).toBe(false)
  })

  it('ignores a trailing slash on either side', () => {
    expect(isActive(`${DEPLOYMENT}/`, DEPLOYMENT, true)).toBe(true)
    expect(isActive(DEPLOYMENT, `${DEPLOYMENT}/`, true)).toBe(true)
  })

  it('does not light up on a sibling', () => {
    expect(isActive(`${DEPLOYMENT}/usage`, `${DEPLOYMENT}/logs`)).toBe(false)
  })
})
