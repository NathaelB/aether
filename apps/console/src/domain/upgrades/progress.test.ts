import { describe, expect, it } from 'vitest'
import { progressOf, type InFlightUpgrade } from './progress'

function inFlight(current: string, steps: string[]): InFlightUpgrade {
  return {
    from: '25.0.0',
    target: steps[steps.length - 1],
    steps,
    current,
    started_at: '2026-09-11T02:00:00Z',
  }
}

describe('progressOf', () => {
  it('says nothing when no upgrade is running', () => {
    expect(progressOf(null)).toBeNull()
    expect(progressOf(undefined)).toBeNull()
  })

  it('counts the step being applied, not the ones already done', () => {
    const steps = ['26.0.0', '27.0.0', '28.0.0']

    expect(progressOf(inFlight('25.0.0', steps))?.step).toBe(1)
    expect(progressOf(inFlight('26.0.0', steps))?.step).toBe(2)
    expect(progressOf(inFlight('27.0.0', steps))?.step).toBe(3)
  })

  /**
   * The last step reports the target as running before the upgrade is marked
   * over. Counting it as a fourth step of three would read as nonsense.
   */
  it('never runs past the end of the path', () => {
    const progress = progressOf(inFlight('28.0.0', ['26.0.0', '27.0.0', '28.0.0']))

    expect(progress?.step).toBe(3)
    expect(progress?.of).toBe(3)
  })

  it('carries where the upgrade started and where it ends', () => {
    const progress = progressOf(inFlight('26.0.0', ['26.0.0', '27.0.0']))

    expect(progress?.from).toBe('25.0.0')
    expect(progress?.to).toBe('27.0.0')
    expect(progress?.startedAt).toBe('2026-09-11T02:00:00Z')
  })

  /** A single hop is still a path, and reads as one step of one. */
  it('handles an upgrade with nothing in between', () => {
    const progress = progressOf(inFlight('25.0.0', ['26.0.0']))

    expect(progress?.step).toBe(1)
    expect(progress?.of).toBe(1)
  })
})
