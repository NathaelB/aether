import { describe, expect, it } from 'vitest'
import { NEW_SIGNATURE_HINT, summarizeSignatures, whyGroupFailed } from './grouping'
import type { Schemas } from '@/api/api.client'

function signature(overrides: Partial<Schemas.LogSignature> = {}): Schemas.LogSignature {
  return {
    fingerprint: 'aaaaaaaaaaaaaaaa',
    count: 1,
    sample_message: 'something went wrong',
    is_new: false,
    ...overrides,
  }
}

describe('summarizeSignatures', () => {
  it('says plainly when nothing matched', () => {
    expect(summarizeSignatures([])).toBe('No signatures')
  })

  it('counts a single signature in the singular', () => {
    expect(summarizeSignatures([signature()])).toBe('1 signature')
  })

  it('counts several signatures in the plural', () => {
    expect(summarizeSignatures([signature(), signature({ fingerprint: 'b' })])).toBe(
      '2 signatures',
    )
  })

  /** The acceptance criterion itself: a new signature is told apart from one that fires every day. */
  it('names how many are new, distinct from the total', () => {
    expect(
      summarizeSignatures([
        signature({ fingerprint: 'a', is_new: true }),
        signature({ fingerprint: 'b', is_new: false }),
        signature({ fingerprint: 'c', is_new: false }),
      ]),
    ).toBe('3 signatures, 1 new')
  })

  it('leaves the new count out once nothing is new', () => {
    expect(summarizeSignatures([signature(), signature({ fingerprint: 'b' })])).toBe(
      '2 signatures',
    )
  })
})

describe('NEW_SIGNATURE_HINT', () => {
  /** The wording must not overclaim: "new" is bounded by the baseline window, not lifetime. */
  it('does not claim a signature was never seen before', () => {
    expect(NEW_SIGNATURE_HINT.toLowerCase()).not.toContain('never seen')
  })

  it('names the baseline explicitly', () => {
    expect(NEW_SIGNATURE_HINT).toMatch(/before this window/)
  })
})

describe('whyGroupFailed', () => {
  it('names the installation gap plainly, distinct from an empty result', () => {
    expect(whyGroupFailed(409)).toMatch(/not set up/)
  })

  it('tells a refusal apart from a missing deployment', () => {
    expect(whyGroupFailed(403)).not.toBe(whyGroupFailed(404))
  })

  it('falls back to the status code for anything unnamed', () => {
    expect(whyGroupFailed(500)).toContain('500')
  })
})
