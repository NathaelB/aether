import { describe, expect, it } from 'vitest'
import {
  accessFrom,
  checkRange,
  describeAccess,
  describeProblem,
  duplicates,
  hasChanges,
  isOpen,
  problems,
  rangesOf,
} from './network-access'

describe('checkRange', () => {
  it('accepts what the platform accepts', () => {
    for (const range of ['203.0.113.0/24', '10.0.0.0/8', '0.0.0.0/0', '192.168.1.1/32']) {
      expect(checkRange(range), range).toBeNull()
    }
  })

  it('accepts v6 ranges by shape and width', () => {
    expect(checkRange('2001:db8::/32')).toBeNull()
    expect(checkRange('::1/128')).toBeNull()
  })

  it('refuses what is not a range at all', () => {
    for (const raw of ['', '203.0.113.0', 'nonsense', '/24', '203.0.113.0/', '203.0.113.0/x']) {
      expect(checkRange(raw), raw).not.toBeNull()
    }
  })

  it('refuses an octet that does not fit in one', () => {
    expect(checkRange('299.0.113.0/24')).toEqual({ kind: 'not-a-range' })
  })

  /**
   * '010.0.0.1' reads as one thing and parses as another in several places.
   * Refused rather than normalised, so what is stored is what was written.
   */
  it('refuses a leading zero rather than quietly reinterpreting it', () => {
    expect(checkRange('010.0.0.0/8')).toEqual({ kind: 'not-a-range' })
  })

  it('refuses a prefix wider than the address family', () => {
    expect(checkRange('203.0.113.0/33')).toEqual({ kind: 'prefix-too-long', width: 32 })
    expect(checkRange('2001:db8::/129')).toEqual({ kind: 'prefix-too-long', width: 128 })
  })

  /**
   * The mistake this catches is somebody pasting their own address and
   * widening the mask. The suggestion is the point: it names what they meant.
   */
  it('refuses host bits and names the network that was meant', () => {
    expect(checkRange('203.0.113.9/24')).toEqual({
      kind: 'host-bits-set',
      network: '203.0.113.0/24',
    })
  })

  it('says the same thing the platform would say', () => {
    const problem = checkRange('203.0.113.9/24')
    expect(problem).not.toBeNull()
    expect(describeProblem(problem!, '203.0.113.9/24')).toContain('203.0.113.0/24')
  })
})

describe('accessFrom', () => {
  /**
   * The rule the whole chantier turns on, at the last place it could be
   * broken. Clearing the last entry is going back to open.
   */
  it('reads an empty form as open, never as restricted to nobody', () => {
    expect(accessFrom([])).toEqual({ kind: 'open' })
    expect(accessFrom([''])).toEqual({ kind: 'open' })
    expect(accessFrom(['  ', ''])).toEqual({ kind: 'open' })
  })

  it('drops the blank row a form always has at the end', () => {
    expect(accessFrom(['10.0.0.0/8', ''])).toEqual({
      kind: 'restricted',
      allowed: ['10.0.0.0/8'],
    })
  })
})

describe('rangesOf and isOpen', () => {
  it('reads what is applied', () => {
    expect(isOpen({ kind: 'open' })).toBe(true)
    expect(isOpen(undefined)).toBe(true)
    expect(rangesOf({ kind: 'open' })).toEqual([])
    expect(rangesOf({ kind: 'restricted', allowed: ['10.0.0.0/8'] })).toEqual(['10.0.0.0/8'])
  })
})

describe('describeAccess', () => {
  it('says a deployment with no ranges is reachable from anywhere', () => {
    expect(describeAccess([], 'auth.acme.com')).toContain('reachable from anywhere')
  })

  /**
   * The warning the screen owes the reader. The console keeps working either
   * way, so nothing stops them -- but nobody should find out afterwards that
   * they are no longer in the set.
   */
  it('warns that every other address is refused, this browser included', () => {
    const sentence = describeAccess(['198.51.100.0/24'], 'auth.acme.com')

    expect(sentence).toContain('auth.acme.com')
    expect(sentence).toContain('including this browser')
  })

  it('counts the ranges rather than saying "these 1 ranges"', () => {
    expect(describeAccess(['10.0.0.0/8'], 'h')).toContain('Only one range')
    expect(describeAccess(['10.0.0.0/8', '203.0.113.0/24'], 'h')).toContain('these 2 ranges')
  })
})

describe('hasChanges', () => {
  it('sees nothing to save when the form matches what is applied', () => {
    const applied = { kind: 'restricted' as const, allowed: ['10.0.0.0/8'] }

    expect(hasChanges(['10.0.0.0/8'], applied)).toBe(false)
    expect(hasChanges(['10.0.0.0/8', ''], applied)).toBe(false)
    expect(hasChanges([], { kind: 'open' })).toBe(false)
  })

  it('sees the change when a range is added, removed or reordered', () => {
    const applied = { kind: 'restricted' as const, allowed: ['10.0.0.0/8', '203.0.113.0/24'] }

    expect(hasChanges(['10.0.0.0/8'], applied)).toBe(true)
    expect(hasChanges(['203.0.113.0/24', '10.0.0.0/8'], applied)).toBe(true)
    expect(hasChanges([], applied)).toBe(true)
  })

  it('sees restricting an open deployment', () => {
    expect(hasChanges(['10.0.0.0/8'], { kind: 'open' })).toBe(true)
  })
})

describe('problems', () => {
  it('marks the rows that are wrong and leaves the blank one alone', () => {
    const found = problems(['10.0.0.0/8', 'nonsense', ''])

    expect(found.size).toBe(1)
    expect(found.get(1)).toEqual({ kind: 'not-a-range' })
  })
})

describe('duplicates', () => {
  /**
   * The platform keeps the first and drops the rest silently. Saying so in
   * the form is the only place a person finds out they wrote it twice.
   */
  it('marks the repeat rather than the first one', () => {
    const repeated = duplicates(['10.0.0.0/8', '203.0.113.0/24', '10.0.0.0/8'])

    expect(repeated.has(2)).toBe(true)
    expect(repeated.has(0)).toBe(false)
  })

  it('says nothing about blank rows', () => {
    expect(duplicates(['', '']).size).toBe(0)
  })
})
