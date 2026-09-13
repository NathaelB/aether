import { describe, expect, it } from 'vitest'
import type { BackupSchedule } from './schedule'
import { checkForm, describe as describeSchedule, formFor, hasChanges, requestFrom } from './schedule'

function schedule(overrides: Partial<BackupSchedule> = {}): BackupSchedule {
  return {
    deployment_id: 'd1',
    organisation_id: 'o1',
    cadence: { every: 'daily', at: '02:30:00' },
    zone: 'Europe/Paris',
    retention: { keep_last: 7, keep_for_days: 30 },
    method: 'physical',
    enabled: true,
    created_at: '2026-09-01T00:00:00Z',
    updated_at: '2026-09-01T00:00:00Z',
    ...overrides,
  } as BackupSchedule
}

describe('reading a schedule into a form', () => {
  it('drops the seconds the input cannot show', () => {
    expect(formFor(schedule()).at).toBe('02:30')
  })

  /// Switching to weekly should land on a real day, not an empty select
  /// nobody can submit.
  it('offers a day even when the cadence has none', () => {
    expect(formFor(schedule()).day).toBe('Sun')
  })

  it('keeps the day a weekly cadence names', () => {
    const weekly = schedule({ cadence: { every: 'weekly', day: 'Wed', at: '04:00:00' } })

    expect(formFor(weekly)).toMatchObject({ every: 'weekly', day: 'Wed', at: '04:00' })
  })
})

describe('writing a form back', () => {
  it('sends the day only when the cadence uses it', () => {
    const daily = requestFrom(formFor(schedule()), 'Europe/Paris')
    expect(daily.day).toBeUndefined()

    const weekly = requestFrom(
      { ...formFor(schedule()), every: 'weekly', day: 'Wed' },
      'Europe/Paris',
    )
    expect(weekly.day).toBe('Wed')
  })

  it('sends retention as numbers, not as what was typed', () => {
    const request = requestFrom({ ...formFor(schedule()), keepLast: '14' }, 'UTC')

    expect(request.keep_last).toBe(14)
    expect(request.keep_for_days).toBe(30)
  })
})

describe('what is wrong with the form', () => {
  it('accepts what the platform accepts', () => {
    expect(checkForm(formFor(schedule()))).toBeNull()
  })

  /// The platform refuses these too -- it does not trust a form -- but a
  /// field that goes red as you leave it beats one that goes red after a
  /// round trip.
  it('says so before the round trip', () => {
    expect(checkForm({ ...formFor(schedule()), at: 'half past two' })).toContain('HH:MM')
    expect(checkForm({ ...formFor(schedule()), keepLast: '0' })).toContain('retention policy')
    expect(checkForm({ ...formFor(schedule()), keepForDays: '-1' })).toContain('backwards')
  })
})

describe('whether the form changed anything', () => {
  it('is false for a form nobody touched', () => {
    expect(hasChanges(formFor(schedule()), schedule())).toBe(false)
  })

  it('is true once a value differs', () => {
    expect(hasChanges({ ...formFor(schedule()), keepLast: '14' }, schedule())).toBe(true)
    expect(hasChanges({ ...formFor(schedule()), enabled: false }, schedule())).toBe(true)
  })

  /// Switching to weekly and back leaves a day behind that changes nothing,
  /// and a Save button that lights up for it is lying.
  it('ignores a day a daily cadence does not use', () => {
    expect(hasChanges({ ...formFor(schedule()), day: 'Wed' }, schedule())).toBe(false)
  })
})

describe('saying what a schedule does', () => {
  it('reads as a sentence rather than as fields', () => {
    expect(describeSchedule(schedule())).toBe(
      'Archived every day at 02:30 Europe/Paris, keeping the last 7 archives and anything from the last 30 days.',
    )
  })

  it('names the day for a weekly one', () => {
    const weekly = schedule({ cadence: { every: 'weekly', day: 'Wed', at: '04:00:00' } })

    expect(describeSchedule(weekly)).toContain('every Wednesday at 04:00')
  })

  it('counts one of something as one', () => {
    const spare = schedule({ retention: { keep_last: 1, keep_for_days: 1 } })

    expect(describeSchedule(spare)).toContain('the last 1 archive and anything from the last 1 day.')
  })

  /// A schedule that is off keeps its settings, and saying when it would run
  /// would read as though it does.
  it('says backups are off rather than when they would be taken', () => {
    expect(describeSchedule(schedule({ enabled: false }))).toBe(
      'Backups are off for this instance.',
    )
  })
})
