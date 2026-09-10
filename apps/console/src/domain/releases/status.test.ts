import { describe, expect, it } from 'vitest'
import { nextStatuses, withdrawalStrands } from './status'

describe('nextStatuses', () => {
  /**
   * A release only ever moves forward. The API refuses the rest, but a button
   * that exists to be rejected is a button that should not be there.
   */
  it('offers only forward steps', () => {
    expect(nextStatuses('upcoming')).toEqual(['available', 'deprecated', 'withdrawn'])
    expect(nextStatuses('available')).toEqual(['deprecated', 'withdrawn'])
    expect(nextStatuses('deprecated')).toEqual(['withdrawn'])
  })

  /**
   * Withdrawn is the end. Nothing to offer, so the control disappears rather
   * than sitting there empty.
   */
  it('offers nothing once a release is withdrawn', () => {
    expect(nextStatuses('withdrawn')).toEqual([])
  })

  /**
   * Skipping deprecated is what an operator does when a version turns out to
   * corrupt data, so the screen has to allow it.
   */
  it('lets an available release be withdrawn directly', () => {
    expect(nextStatuses('available')).toContain('withdrawn')
  })
})

describe('withdrawalStrands', () => {
  /**
   * Withdrawing a version nothing runs is housekeeping. Withdrawing one that
   * is still serving traffic is an incident, and the screen should say so
   * before the click rather than after.
   */
  it('warns when a withdrawn version is still serving', () => {
    expect(withdrawalStrands('withdrawn', 3)).toBe(true)
  })

  it('stays quiet when nothing runs it', () => {
    expect(withdrawalStrands('withdrawn', 0)).toBe(false)
  })

  it('stays quiet for a version that is merely deprecated', () => {
    expect(withdrawalStrands('deprecated', 12)).toBe(false)
    expect(withdrawalStrands('available', 12)).toBe(false)
  })
})
