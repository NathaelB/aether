import { describe, expect, it } from 'vitest'
import type { Schemas } from '@/api/api.client'
import {
  audienceLabel,
  installableVersions,
  isGloballyAvailable,
  newestInstallable,
  nextPatchIn,
  releaseLines,
} from './catalogue'

function release(
  version: string,
  status: Schemas.ReleaseStatus = 'available',
  rollout: Schemas.Rollout = { percentage: 100, pilot_organisations: [] },
): Schemas.Release {
  return {
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    id: { kind: 'ferriskey', version },
    notes: '',
    risk: 'none',
    status,
    steps_through: [],
    rollout,
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

describe('isGloballyAvailable', () => {
  it('is true only when nothing is held back', () => {
    expect(isGloballyAvailable(release('26.7.3'))).toBe(true)
  })

  /**
   * Publishing puts a version in the catalogue; it does not hand it to the
   * estate. A release arrives offered to nobody.
   */
  it('is false for a release offered to nobody', () => {
    const closed = release('26.7.3', 'available', { percentage: 0, pilot_organisations: [] })

    expect(isGloballyAvailable(closed)).toBe(false)
  })

  it('is false while a rollout is still partial', () => {
    const half = release('26.7.3', 'available', { percentage: 50, pilot_organisations: [] })
    const plans = release('26.7.3', 'available', {
      percentage: 100,
      pilot_organisations: [],
      plans: ['Enterprise'],
    })

    expect(isGloballyAvailable(half)).toBe(false)
    expect(isGloballyAvailable(plans)).toBe(false)
  })
})

describe('releaseLines', () => {
  it('groups patches under the line they continue', () => {
    const lines = releaseLines([
      release('26.7.0'),
      release('26.6.4'),
      release('26.7.3'),
      release('26.6.1'),
    ])

    expect(lines.map((line) => line.label)).toEqual(['26.7', '26.6'])
    expect(lines[0].releases.map((r) => r.id.version)).toEqual(['26.7.3', '26.7.0'])
    expect(lines[1].releases.map((r) => r.id.version)).toEqual(['26.6.4', '26.6.1'])
  })

  /** String order puts 10 before 9, in the lines as in the versions. */
  it('orders lines by number', () => {
    const lines = releaseLines([release('26.9.0'), release('26.10.0')])

    expect(lines.map((line) => line.label)).toEqual(['26.10', '26.9'])
  })

  it('leaves out anything that is not a version', () => {
    expect(releaseLines([release('latest')])).toEqual([])
  })
})

describe('nextPatchIn', () => {
  it('offers the patch that continues the line', () => {
    const [line] = releaseLines([release('26.7.0'), release('26.7.3')])

    expect(nextPatchIn(line)).toBe('26.7.4')
  })

  it('offers nothing for a line that holds nothing', () => {
    expect(nextPatchIn({ label: '26.7', releases: [] })).toBeNull()
  })
})

describe('audienceLabel', () => {
  it('says plainly when everybody has it', () => {
    expect(audienceLabel({ percentage: 100, pilot_organisations: [] })).toBe('Everyone')
  })

  /** The state a release is published in, and the one worth noticing. */
  it('says plainly when nobody does', () => {
    expect(audienceLabel({ percentage: 0, pilot_organisations: [] })).toBe('Nobody yet')
  })

  it('describes a rollout that is under way', () => {
    expect(audienceLabel({ percentage: 25, pilot_organisations: [] })).toBe('25% of the estate')
    expect(audienceLabel({ percentage: 50, pilot_organisations: [], plans: ['Enterprise'] })).toBe(
      '50% of the estate on Enterprise',
    )
  })

  /** Pilots reach it whatever the percentage says, so the label has to admit it. */
  it('counts the pilots even at nothing percent', () => {
    expect(audienceLabel({ percentage: 0, pilot_organisations: ['a', 'b'] })).toBe(
      '0% of the estate, plus 2 pilots',
    )
  })
})
