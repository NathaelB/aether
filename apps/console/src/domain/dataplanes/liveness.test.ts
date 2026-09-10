import { describe, expect, it } from 'vitest'
import { liveness } from './liveness'

const now = new Date('2026-09-10T12:00:00Z')
const minutesAgo = (minutes: number) =>
  new Date(now.getTime() - minutes * 60 * 1000).toISOString()

describe('liveness', () => {
  it('is reachable when it reported recently', () => {
    expect(liveness({ last_seen_at: minutesAgo(1) }, now)).toBe('reachable')
  })

  it('is stale once it has been quiet for long enough', () => {
    expect(liveness({ last_seen_at: minutesAgo(30) }, now)).toBe('stale')
  })

  /**
   * Never having reported is a different situation from having stopped: one is
   * a data plane still coming up, the other is one that broke. The Rust domain
   * makes the same distinction, and collapsing it would show a freshly
   * provisioned cluster as a failure.
   */
  it.each([null, undefined])('distinguishes never having reported (%o)', (value) => {
    expect(liveness({ last_seen_at: value }, now)).toBe('never-seen')
  })

  it('treats an unparseable timestamp as never seen rather than as reachable', () => {
    expect(liveness({ last_seen_at: 'not a date' }, now)).toBe('never-seen')
  })

  /**
   * The threshold errs deliberately late: the control plane owns the real
   * heartbeat window and the console is not told what it is. Lagging behind it
   * says less than it could; contradicting it would be worse.
   */
  it('stays reachable well past any plausible server-side window', () => {
    expect(liveness({ last_seen_at: minutesAgo(2) }, now)).toBe('reachable')
  })
})
