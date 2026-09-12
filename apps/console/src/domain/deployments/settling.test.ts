import { describe, expect, it } from 'vitest'
import { anySettling, isSettling, settlingRefetchInterval } from './settling'

describe('isSettling', () => {
  /**
   * The bug this closes. An upgrade moves the version without anyone asking
   * the console again, so the record on screen is stale for as long as it
   * runs -- and the screen offers an upgrade to the version being applied.
   */
  it('counts a deployment being upgraded as still moving', () => {
    expect(isSettling('upgrading')).toBe(true)
  })

  it('counts a deployment coming up or being torn down as still moving', () => {
    expect(isSettling('pending')).toBe(true)
    expect(isSettling('scheduling')).toBe(true)
    expect(isSettling('in_progress')).toBe(true)
    expect(isSettling('deleting')).toBe(true)
  })

  /**
   * A resting state changes only when someone acts on it, and that act
   * refreshes the record itself. Polling one for ever would be a request a
   * second that can never report anything new.
   */
  it('leaves a deployment at rest alone', () => {
    expect(isSettling('successful')).toBe(false)
    expect(isSettling('failed')).toBe(false)
    expect(isSettling('deleted')).toBe(false)
    expect(isSettling('maintenance')).toBe(false)
    expect(isSettling('upgrade_required')).toBe(false)
  })

  it('says nothing about a deployment it has not read yet', () => {
    expect(isSettling(undefined)).toBe(false)
    expect(isSettling(null)).toBe(false)
  })
})

describe('settlingRefetchInterval', () => {
  /**
   * Polling has to stop on the reply that reports a resting state, because
   * that same reply carries the version the upgrade landed on. Nothing else
   * watches for the end of an upgrade.
   */
  it('stops on the reply that reports a resting state', () => {
    expect(settlingRefetchInterval('successful')).toBe(false)
  })

  it('asks again while the platform is still moving it', () => {
    const interval = settlingRefetchInterval('upgrading')

    expect(interval).toBeGreaterThan(0)
    expect(interval).toBeLessThanOrEqual(10_000)
  })
})

describe('anySettling', () => {
  it('keeps a list fresh while one of its deployments is moving', () => {
    expect(anySettling([{ status: 'successful' }, { status: 'upgrading' }])).toBeGreaterThan(0)
  })

  it('stops once every deployment in the list is at rest', () => {
    expect(anySettling([{ status: 'successful' }, { status: 'failed' }])).toBe(false)
    expect(anySettling([])).toBe(false)
    expect(anySettling(undefined)).toBe(false)
  })
})
