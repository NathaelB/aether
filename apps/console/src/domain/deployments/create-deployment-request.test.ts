import { describe, expect, it } from 'vitest'
import {
  toCreateDeploymentRequest,
  toNamespace,
  type CreateDeploymentForm,
} from './create-deployment-request'
import { DEPLOYMENT_PLANS } from './types/deployment'

const form = (overrides: Partial<CreateDeploymentForm> = {}): CreateDeploymentForm => ({
  name: 'auth',
  type: 'ferriskey',
  environment: 'production',
  region: 'local',
  mode: 'shared',
  plan: 'starter',
  ...overrides,
})

describe('toCreateDeploymentRequest', () => {
  /**
   * The regression this whole workstream exists for. The form collected a
   * region, a plan and a mode, and the request carried none of them, so every
   * deployment landed shared, in the default region, at the default size --
   * whatever the user had picked.
   */
  it('sends every choice the form collected', () => {
    const request = toCreateDeploymentRequest(
      form({ region: 'eu-west-1', mode: 'dedicated', plan: 'premium' }),
    )

    expect(request.region).toBe('eu-west-1')
    expect(request.mode).toBe('dedicated')
    expect(request.cpu_millis).toBe(4000)
    expect(request.memory_mib).toBe(8192)
    expect(request.storage_gib).toBe(20)
  })

  /**
   * The API rejects a request that sets CPU without storage rather than
   * completing it from a default, on the grounds that a half-specified size is
   * more likely a mistake than an intent. Every plan must therefore carry all
   * three numbers -- this checks the data, not the function.
   */
  it.each(Object.keys(DEPLOYMENT_PLANS) as (keyof typeof DEPLOYMENT_PLANS)[])(
    'sends all three dimensions for the %s plan',
    (plan) => {
      const request = toCreateDeploymentRequest(form({ plan }))

      expect(request.cpu_millis).toBeGreaterThan(0)
      expect(request.memory_mib).toBeGreaterThan(0)
      expect(request.storage_gib).toBeGreaterThan(0)
    },
  )

  it('maps the identity provider onto a kind the control plane knows', () => {
    expect(toCreateDeploymentRequest(form({ type: 'ferriskey' })).kind).toBe('ferriskey')
    expect(toCreateDeploymentRequest(form({ type: 'keycloak' })).kind).toBe('keycloak')
    // authentik is offered by the form and not implemented by the control
    // plane. Until it is, it deploys Keycloak -- which the form has always
    // done silently.
    expect(toCreateDeploymentRequest(form({ type: 'authentik' })).kind).toBe('keycloak')
  })

  it('defaults nothing on the client side', () => {
    // The control plane substitutes its own default region for an absent one
    // and never for a named one. Sending an empty string instead of omitting
    // the field would defeat that, so the form must always carry a region.
    const request = toCreateDeploymentRequest(form({ region: 'local' }))

    expect(request.region).toBe('local')
  })
})

describe('toNamespace', () => {
  it('joins the environment and the name', () => {
    expect(toNamespace('production', 'auth')).toBe('production-auth')
  })

  it('strips accents rather than the letters carrying them', () => {
    expect(toNamespace('development', 'déveloped')).toBe('development-developed')
  })

  it.each([
    ['Auth Service', 'production-auth-service'],
    ['l\'auth', 'production-l-auth'],
    ['auth__service', 'production-auth-service'],
    ['  auth  ', 'production-auth'],
  ])('turns %o into a DNS-1123 label', (name, expected) => {
    expect(toNamespace('production', name)).toBe(expected)
  })

  /** A namespace is at most 63 characters, and may not end in a hyphen. */
  it('truncates a long name without leaving a trailing hyphen', () => {
    const namespace = toNamespace('production', `${'a'.repeat(50)} ${'b'.repeat(50)}`)

    expect(namespace.length).toBeLessThanOrEqual(63)
    expect(namespace).not.toMatch(/-$/)
    expect(namespace).toMatch(/^[a-z0-9]([a-z0-9-]*[a-z0-9])?$/)
  })

  it('never starts or ends with a hyphen', () => {
    expect(toNamespace('production', '---auth---')).toBe('production-auth')
  })
})
