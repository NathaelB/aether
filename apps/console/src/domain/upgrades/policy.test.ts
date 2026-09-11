import { describe, expect, it } from 'vitest'
import { describeWindow, offeredTimezones, policyIsInert } from './policy'

describe('policyIsInert', () => {
  /**
   * The trap the screen exists to close. A customer picks "patches only",
   * leaves the window empty, and has chosen a setting that does nothing. The
   * API is right to refuse to act; the screen has to say so before they leave.
   */
  it('flags a policy that will never do anything without a window', () => {
    expect(policyIsInert('patch', false)).toBe(true)
    expect(policyIsInert('patch_and_minor', false)).toBe(true)
  })

  it('says nothing when there is a window to act in', () => {
    expect(policyIsInert('patch', true)).toBe(false)
    expect(policyIsInert('patch_and_minor', true)).toBe(false)
  })

  /**
   * Manual with no window is not a contradiction, it is the default. Warning
   * about it would train people to ignore the warning.
   */
  it('says nothing about a manual policy', () => {
    expect(policyIsInert('manual', false)).toBe(false)
    expect(policyIsInert('manual', true)).toBe(false)
  })
})

describe('describeWindow', () => {
  it('reads back what was chosen, in the zone it was chosen in', () => {
    expect(
      describeWindow({ day: 'sun', start: '03:00', duration: 120, timezone: 'Europe/Paris' }),
    ).toBe('Sunday at 03:00 for 2h, Europe/Paris')
  })

  it('keeps minutes that do not make a whole hour', () => {
    expect(
      describeWindow({ day: 'mon', start: '01:30', duration: 90, timezone: 'UTC' }),
    ).toBe('Monday at 01:30 for 1h 30m, UTC')

    expect(
      describeWindow({ day: 'mon', start: '01:30', duration: 45, timezone: 'UTC' }),
    ).toBe('Monday at 01:30 for 45m, UTC')
  })

  /**
   * No window is a state worth a sentence rather than an empty line: it is the
   * reason nothing is being applied.
   */
  it('says what no window means rather than leaving a blank', () => {
    expect(describeWindow(null)).toContain('nothing is applied automatically')
  })
})

describe('offeredTimezones', () => {
  /**
   * A free text field accepts anything and fails at the API. The list starts
   * with the viewer's own zone so the common case takes no thought.
   */
  it('offers the browser zone first and never repeats it', () => {
    const offered = offeredTimezones()

    expect(offered.length).toBeGreaterThan(1)
    expect(new Set(offered).size).toBe(offered.length)
    expect(offered).toContain('UTC')
  })
})
