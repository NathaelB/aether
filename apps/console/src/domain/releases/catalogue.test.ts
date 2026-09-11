import { describe, expect, it } from 'vitest'
import type { Schemas } from '@/api/api.client'
import { installableVersions, newestInstallable } from './catalogue'

function release(version: string, status: Schemas.ReleaseStatus = 'available'): Schemas.Release {
  return {
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    id: { kind: 'ferriskey', version },
    notes: '',
    risk: 'none',
    status,
    steps_through: [],
    rollout: { percentage: 100, pilot_organisations: [] },
  }
}

describe('installableVersions', () => {
  it('offers the newest first', () => {
    const offered = installableVersions([release('26.0.1'), release('26.2.0'), release('26.1.0')])

    expect(offered.map((r) => r.id.version)).toEqual(['26.2.0', '26.1.0', '26.0.1'])
  })

  /** String order puts 10 before 9, which is how a form offers a stale default. */
  it('orders by number, not by text', () => {
    const offered = installableVersions([release('26.0.9'), release('26.0.10')])

    expect(offered[0].id.version).toBe('26.0.10')
  })

  /**
   * Starting a new instance on a deprecated version buys something already on
   * its way out; planned and withdrawn cannot be installed at all.
   */
  it('offers nothing that is not available', () => {
    const offered = installableVersions([
      release('26.1.0', 'deprecated'),
      release('26.2.0', 'upcoming'),
      release('26.3.0', 'withdrawn'),
    ])

    expect(offered).toEqual([])
  })
})

describe('newestInstallable', () => {
  it('names what the form should default to', () => {
    expect(newestInstallable([release('26.0.1'), release('26.1.0')])).toBe('26.1.0')
  })

  /**
   * An empty catalogue is the state the console has to speak about: without a
   * version there is nothing to create, and the API refuses with a message
   * about `latest` that says nothing about the real cause.
   */
  it('says nothing when the catalogue holds nothing installable', () => {
    expect(newestInstallable([])).toBeNull()
    expect(newestInstallable([release('26.1.0', 'upcoming')])).toBeNull()
  })
})
