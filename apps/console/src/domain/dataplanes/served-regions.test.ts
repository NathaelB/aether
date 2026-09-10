import { describe, expect, it } from 'vitest'
import type { Schemas } from '@/api/api.client'
import { servedRegions } from './served-regions'

const dataplane = (
  region: string,
  status: Schemas.DataPlaneStatus = 'Active',
): Schemas.DataPlane => ({
  id: `id-${region}-${status}`,
  region,
  status,
  allocation: 'Shared',
  capacity: { cpu_millis: 8000, memory_mib: 16384, storage_gib: 100 },
  last_seen_at: null,
  created_at: '2026-01-01T00:00:00Z',
})

describe('servedRegions', () => {
  it('offers nothing when no data plane exists', () => {
    expect(servedRegions([])).toEqual([])
  })

  it('lists each region once, however many data planes serve it', () => {
    expect(servedRegions([dataplane('local'), dataplane('local'), dataplane('fr-par')])).toEqual([
      'fr-par',
      'local',
    ])
  })

  /**
   * A region whose only data planes cannot serve is worse than no choice: the
   * user picks it, and placement fails on the other side of a form submission.
   */
  it.each(['Disabled', 'Failed'] as const)('does not offer a region served only by a %s plane', (status) => {
    expect(servedRegions([dataplane('fr-par', status)])).toEqual([])
  })

  /**
   * Provisioning and draining are deliberately still offered. A provisioning
   * plane is one coming up, and a draining one still runs what is on it --
   * neither is a reason to hide the region, which may be served by others.
   */
  it.each(['Provisioning', 'Draining'] as const)('still offers a region served by a %s plane', (status) => {
    expect(servedRegions([dataplane('fr-par', status)])).toEqual(['fr-par'])
  })

  it('keeps a region that has one healthy plane and one failed', () => {
    expect(servedRegions([dataplane('local', 'Failed'), dataplane('local', 'Active')])).toEqual([
      'local',
    ])
  })
})
