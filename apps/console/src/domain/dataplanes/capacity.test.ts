import { describe, expect, it } from 'vitest'
import type { Schemas } from '@/api/api.client'
import { allocationLabel, allocationOwner, capacityUsage } from './capacity'

const capacity: Schemas.Capacity = { cpu_millis: 4000, memory_mib: 8192, storage_gib: 100 }

const deployment = (
  resources: Schemas.DeploymentResources,
  overrides: Partial<Schemas.Deployment> = {},
): Schemas.Deployment =>
  ({
    id: 'deployment',
    organisation_id: 'org',
    dataplane_id: 'dp',
    name: 'auth',
    kind: 'ferriskey',
    version: 'latest',
    status: 'successful',
    namespace: 'production-auth',
    resources,
    created_by: 'user',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }) as Schemas.Deployment

describe('capacityUsage', () => {
  it('reports an empty data plane as entirely free', () => {
    const usage = capacityUsage(capacity, [])

    expect(usage.cpuMillis).toEqual({ used: 0, total: 4000, percent: 0 })
    expect(usage.storageGib.percent).toBe(0)
  })

  it('adds up every dimension independently', () => {
    const usage = capacityUsage(capacity, [
      deployment({ cpu_millis: 1000, memory_mib: 2048, storage_gib: 5 }),
      deployment({ cpu_millis: 1000, memory_mib: 1024, storage_gib: 20 }),
    ])

    expect(usage.cpuMillis).toEqual({ used: 2000, total: 4000, percent: 50 })
    expect(usage.memoryMib).toEqual({ used: 3072, total: 8192, percent: 38 })
    expect(usage.storageGib).toEqual({ used: 25, total: 100, percent: 25 })
  })

  /**
   * A pending deployment has already been placed here. Its room is spoken for,
   * and showing it as free would misrepresent what the next placement can use.
   */
  it('counts a pending deployment against capacity', () => {
    const usage = capacityUsage(capacity, [
      deployment({ cpu_millis: 2000, memory_mib: 1024, storage_gib: 1 }, { status: 'pending' }),
    ])

    expect(usage.cpuMillis.used).toBe(2000)
  })

  it('does not count a deleted deployment', () => {
    const usage = capacityUsage(capacity, [
      deployment(
        { cpu_millis: 2000, memory_mib: 1024, storage_gib: 1 },
        { deleted_at: '2026-01-02T00:00:00Z' },
      ),
    ])

    expect(usage.cpuMillis.used).toBe(0)
  })

  /** The bar must not run off the card, whatever the numbers say. */
  it('clamps an over-subscribed dimension at 100%', () => {
    const usage = capacityUsage(capacity, [
      deployment({ cpu_millis: 99000, memory_mib: 1024, storage_gib: 1 }),
    ])

    expect(usage.cpuMillis.percent).toBe(100)
    expect(usage.cpuMillis.used).toBe(99000)
  })

  /** A response is data, not a promise. Dividing by it would print `NaN%`. */
  it('does not divide by a zero capacity', () => {
    const usage = capacityUsage({ cpu_millis: 0, memory_mib: 0, storage_gib: 0 }, [
      deployment({ cpu_millis: 500, memory_mib: 512, storage_gib: 1 }),
    ])

    expect(usage.cpuMillis.percent).toBe(0)
    expect(Number.isNaN(usage.memoryMib.percent)).toBe(false)
  })

  /**
   * #270: absent means exactly today's behaviour, so nothing here invents a
   * ceiling nobody set.
   */
  it('has no deployment-count usage when the data plane carries no bound', () => {
    const usage = capacityUsage(capacity, [
      deployment({ cpu_millis: 500, memory_mib: 1024, storage_gib: 1 }),
    ])

    expect(usage.deploymentCount).toBeNull()
  })

  /** The point of the issue: a second bound alongside the resources. */
  it('reports deployment-count usage against the bound when one is set', () => {
    const usage = capacityUsage({ ...capacity, max_deployments: 3 }, [
      deployment({ cpu_millis: 500, memory_mib: 1024, storage_gib: 1 }),
      deployment({ cpu_millis: 500, memory_mib: 1024, storage_gib: 1 }),
    ])

    expect(usage.deploymentCount).toEqual({ used: 2, total: 3, percent: 67 })
  })

  it('does not count a deleted deployment against the deployment-count bound', () => {
    const usage = capacityUsage({ ...capacity, max_deployments: 3 }, [
      deployment(
        { cpu_millis: 500, memory_mib: 1024, storage_gib: 1 },
        { deleted_at: '2026-01-02T00:00:00Z' },
      ),
    ])

    expect(usage.deploymentCount).toEqual({ used: 0, total: 3, percent: 0 })
  })
})

describe('allocation', () => {
  it('reads the owner out of a dedicated allocation', () => {
    expect(allocationOwner({ dedicated: { organisation_id: 'org-1' } })).toBe('org-1')
    expect(allocationLabel({ dedicated: { organisation_id: 'org-1' } })).toBe('Dedicated')
  })

  it('has no owner for a shared allocation', () => {
    expect(allocationOwner('shared')).toBeNull()
    expect(allocationLabel('shared')).toBe('Shared')
  })
})
