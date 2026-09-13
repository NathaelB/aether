import { describe, expect, it } from 'vitest'
import type { Backup } from './schedule'
import {
  describeProtection,
  newestFirst,
  readableAge,
  readableDuration,
  readableSize,
  summarise,
} from './archives'

const NOW = new Date('2026-09-13T12:00:00Z').getTime()

function archive(overrides: Partial<Backup> = {}): Backup {
  return {
    id: 'b1',
    deployment_id: 'd1',
    organisation_id: 'o1',
    release: { kind: 'keycloak', version: '26.0.0' },
    postgres_major: 17,
    method: 'physical',
    protection: { kind: 'store_managed' },
    location: 'o1/d1/aether/nightly.json',
    size_bytes: 4096,
    started_at: '2026-09-13T02:30:00Z',
    finished_at: '2026-09-13T02:34:00Z',
    ...overrides,
  } as Backup
}

describe('the size of an archive', () => {
  it('is read in the units somebody came for', () => {
    expect(readableSize(512)).toBe('512 B')
    expect(readableSize(4096)).toBe('4.0 kB')
    expect(readableSize(3 * 1024 ** 3)).toBe('3.0 GB')
  })

  /// "1.4 GB" is a size. "1.4382 GB" is a measurement, and the extra digits
  /// are noise on a screen somebody is scanning.
  it('drops the decimal once the number is large enough not to need it', () => {
    expect(readableSize(15.7 * 1024 ** 2)).toBe('16 MB')
  })

  it('says nothing rather than something wrong about a size it cannot read', () => {
    expect(readableSize(Number.NaN)).toBe('—')
    expect(readableSize(-1)).toBe('—')
  })
})

describe('how long an archive took', () => {
  it('is said in the units it actually took', () => {
    expect(readableDuration('2026-09-13T02:30:00Z', '2026-09-13T02:30:12Z')).toBe('12s')
    expect(readableDuration('2026-09-13T02:30:00Z', '2026-09-13T02:34:30Z')).toBe('4m 30s')
    expect(readableDuration('2026-09-13T02:00:00Z', '2026-09-13T03:20:00Z')).toBe('1h 20m')
  })
})

describe('how old an archive is', () => {
  it('is relative, because that is the question the screen answers', () => {
    expect(readableAge('2026-09-13T11:30:00Z', NOW)).toBe('30 minutes ago')
    expect(readableAge('2026-09-13T02:00:00Z', NOW)).toBe('10 hours ago')
    expect(readableAge('2026-09-11T02:00:00Z', NOW)).toBe('2 days ago')
  })

  /// Clocks disagree. An archive stamped a few seconds in the future is a
  /// clock, not a fact about the future, and "in -3 seconds" helps nobody.
  it('reads a stamp from the future as just now', () => {
    expect(readableAge('2026-09-13T12:00:30Z', NOW)).toBe('just now')
  })
})

describe('the list', () => {
  it('puts the newest first whatever order they arrived in', () => {
    const ordered = newestFirst([
      archive({ id: 'old', finished_at: '2026-09-11T02:34:00Z' }),
      archive({ id: 'new', finished_at: '2026-09-13T02:34:00Z' }),
      archive({ id: 'middle', finished_at: '2026-09-12T02:34:00Z' }),
    ])

    expect(ordered.map((one) => one.id)).toEqual(['new', 'middle', 'old'])
  })

  it('does not reorder the array it was given', () => {
    const given = [
      archive({ id: 'old', finished_at: '2026-09-11T02:34:00Z' }),
      archive({ id: 'new', finished_at: '2026-09-13T02:34:00Z' }),
    ]

    newestFirst(given)

    expect(given.map((one) => one.id)).toEqual(['old', 'new'])
  })
})

describe('the line above the list', () => {
  /// A young deployment and a deployment nobody is backing up look the same
  /// in the list and are not the same thing.
  it('tells a deployment with nothing yet from one nobody is archiving', () => {
    expect(summarise([], true, NOW)).toContain('next scheduled run')
    expect(summarise([], false, NOW)).toContain('backups are off')
  })

  it('leads with how old the newest one is', () => {
    const line = summarise(
      [
        archive({ finished_at: '2026-09-11T02:34:00Z' }),
        archive({ finished_at: '2026-09-13T02:34:00Z' }),
      ],
      true,
      NOW,
    )

    expect(line).toContain('2 archives')
    expect(line).toContain('9 hours ago')
  })

  it('counts one archive as one', () => {
    expect(summarise([archive()], true, NOW)).toContain('1 archive,')
  })
})

describe('what protects an archive', () => {
  /// The question a customer is actually asking is whether this platform can
  /// read their data, not which algorithm was used.
  it('says who could read it, not how it was written', () => {
    expect(describeProtection(archive())).toBe('Encrypted by the object store')
    expect(
      describeProtection(
        archive({
          protection: {
            kind: 'envelope',
            provider: 'platform',
            name: 'aether-backups',
            version: 1,
          },
        } as Partial<Backup>),
      ),
    ).toBe('Encrypted before it left the cluster')
  })
})
