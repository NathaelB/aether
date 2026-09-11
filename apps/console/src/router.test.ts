import { describe, expect, it } from 'vitest'
import { router } from './router'

const ORGANISATION = '/organisations/org-1'
const DEPLOYMENT = `${ORGANISATION}/deployments/dep-1`

/**
 * The components a URL puts on screen, outermost first.
 *
 * Read from the tree rather than from a rendered page: what matters here is
 * which shells a path resolves to, and asking the router that directly needs
 * no browser to be standing up.
 */
function shellsFor(pathname: string): string[] {
  return router
    .getMatchedRoutes(pathname)
    .matchedRoutes.map((route) => route.options?.component)
    .filter((component) => typeof component === 'function')
    .map((component) => (component as { name?: string }).name ?? '')
}

describe('the route tree', () => {
  it('gives an organisation page the organisation shell', () => {
    expect(shellsFor(`${ORGANISATION}/deployments`)).toContain('AppLayout')
  })

  /**
   * The bug this exists to keep out: nested, the deployment's header rendered
   * inside the organisation's, and the page opened with two stacked headers
   * and two rows of tabs.
   */
  it('replaces the organisation shell rather than nesting inside it', () => {
    const shells = shellsFor(DEPLOYMENT)

    expect(shells).toContain('DeploymentLayout')
    expect(shells).not.toContain('AppLayout')
  })

  it('keeps the deployment shell on every page beneath it', () => {
    for (const path of ['/logs', '/usage', '/settings', '/settings/version']) {
      const shells = shellsFor(`${DEPLOYMENT}${path}`)

      expect(shells, path).toContain('DeploymentLayout')
      expect(shells, path).not.toContain('AppLayout')
    }
  })

  it('puts the settings navigation only on settings pages', () => {
    expect(shellsFor(`${DEPLOYMENT}/settings`)).toContain('DeploymentSettingsLayout')
    expect(shellsFor(`${DEPLOYMENT}/logs`)).not.toContain('DeploymentSettingsLayout')
  })

  /**
   * `create` and a deployment id sit at the same depth, so the tree has to
   * prefer the literal. Ranked the other way, creating a deployment would
   * open a deployment called "create".
   */
  it('reads /deployments/create as the form, not as a deployment', () => {
    const shells = shellsFor(`${ORGANISATION}/deployments/create`)

    expect(shells).toContain('AppLayout')
    expect(shells).not.toContain('DeploymentLayout')
  })

  it('gives the platform pages their own shell, outside any organisation', () => {
    const shells = shellsFor('/platform/dataplanes')

    expect(shells).toContain('PlatformLayout')
    expect(shells).not.toContain('AppLayout')
  })
})
