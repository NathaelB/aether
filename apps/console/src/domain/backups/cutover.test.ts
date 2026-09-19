import { describe, expect, it } from 'vitest'
import type { Deployment } from './cutover'
import { describeCutover, otherDeployments } from './cutover'

function deployment(overrides: Partial<Deployment> = {}): Deployment {
  return {
    id: 'd1',
    organisation_id: 'o1',
    dataplane_id: 'p1',
    name: 'acme-prod',
    kind: 'keycloak',
    version: '26.0.0',
    status: 'successful',
    namespace: 'production-acme-prod',
    environment: 'production',
    network_access: { kind: 'open' },
    resources: { cpu_millis: 500, memory_mib: 1024, storage_gib: 5 },
    created_by: 'u1',
    created_at: '2026-09-01T00:00:00Z',
    updated_at: '2026-09-01T00:00:00Z',
    auto_upgrade: { kind: 'manual' },
    ...overrides,
  } as Deployment
}

describe('who a deployment could cut over with', () => {
  it('excludes itself', () => {
    const deployments = [deployment({ id: 'd1' }), deployment({ id: 'd2' })]

    expect(otherDeployments(deployments, 'd1').map((one) => one.id)).toEqual(['d2'])
  })

  it('excludes a deleted deployment: there is no hostname left to trade', () => {
    const deployments = [
      deployment({ id: 'd2', deleted_at: '2026-09-10T00:00:00Z' }),
      deployment({ id: 'd3' }),
    ]

    expect(otherDeployments(deployments, 'd1').map((one) => one.id)).toEqual(['d3'])
  })
})

describe('what a cutover does', () => {
  it('names both deployments and both new names, symmetrically', () => {
    const self = deployment({ id: 'd1', name: 'acme-recovery' })
    const other = deployment({ id: 'd2', name: 'acme-prod' })

    const description = describeCutover(self, other)

    expect(description).toContain('acme-prod')
    expect(description).toContain('acme-recovery')
  })
})
