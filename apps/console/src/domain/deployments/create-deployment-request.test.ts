import { describe, expect, it } from 'vitest'
import {
  toCreateDeploymentRequest,
  toNamespace,
  type CreateDeploymentForm,
} from './create-deployment-request'
import { DEPLOYMENT_SIZES, type DeploymentSize } from './types/deployment'

const form = (overrides: Partial<CreateDeploymentForm> = {}): CreateDeploymentForm => ({
  name: 'auth',
  kind: 'ferriskey',
  environment: 'production',
  region: 'local',
  mode: 'shared',
  size: 'small',
  ...overrides,
})

describe('toCreateDeploymentRequest', () => {
  it('sends every choice the form collected', () => {
    const request = toCreateDeploymentRequest(
      form({ region: 'eu-west-1', mode: 'dedicated', size: 'large', kind: 'keycloak' }),
    )

    expect(request.region).toBe('eu-west-1')
    expect(request.mode).toBe('dedicated')
    expect(request.kind).toBe('keycloak')
    expect(request.cpu_millis).toBe(4000)
    expect(request.memory_mib).toBe(8192)
    expect(request.storage_gib).toBe(50)
  })

  // The API rejects a partial size rather than completing it from a default.
  it.each(Object.keys(DEPLOYMENT_SIZES) as DeploymentSize[])(
    'sends all three dimensions for %s',
    (size) => {
      const request = toCreateDeploymentRequest(form({ size }))

      expect(request.cpu_millis).toBeGreaterThan(0)
      expect(request.memory_mib).toBeGreaterThan(0)
      expect(request.storage_gib).toBeGreaterThan(0)
    },
  )

  it('never substitutes a region', () => {
    expect(toCreateDeploymentRequest(form({ region: 'local' })).region).toBe('local')
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
    ['---auth---', 'production-auth'],
  ])('turns %o into a DNS-1123 label', (name, expected) => {
    expect(toNamespace('production', name)).toBe(expected)
  })

  it('truncates a long name without leaving a trailing hyphen', () => {
    const namespace = toNamespace('production', `${'a'.repeat(50)} ${'b'.repeat(50)}`)

    expect(namespace.length).toBeLessThanOrEqual(63)
    expect(namespace).toMatch(/^[a-z0-9]([a-z0-9-]*[a-z0-9])?$/)
  })
})
