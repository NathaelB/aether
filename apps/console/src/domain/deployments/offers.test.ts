import { describe, expect, it } from 'vitest'
import type { Offer, OfferAvailability } from './offers'
import { OFFER_COPY, describeResources, firstOpen, whyClosed } from './offers'

function entry(overrides: Partial<OfferAvailability> = {}): OfferAvailability {
  return {
    offer: 'standard',
    open: true,
    resources: { cpu_millis: 500, memory_mib: 1024, storage_gib: 1 },
    shares_a_cluster: true,
    ...overrides,
  } as OfferAvailability
}

describe('the copy', () => {
  /**
   * The platform decides which offers exist. A console that knows about three
   * of four renders an empty card for the fourth, and nothing fails.
   */
  it('covers every offer the platform can send', () => {
    const known: Offer[] = ['sandbox', 'standard', 'scale', 'private']

    for (const offer of known) {
      expect(OFFER_COPY[offer], offer).toBeDefined()
      expect(OFFER_COPY[offer].label.length).toBeGreaterThan(0)
    }
  })
})

describe('why an offer cannot be chosen', () => {
  it('says nothing about an offer that can', () => {
    expect(whyClosed(entry())).toBeNull()
  })

  /**
   * Hiding a closed offer makes an upsell invisible; showing it without
   * saying what opens it makes the reader guess between four tiers.
   */
  it('names the tier that would open it', () => {
    expect(whyClosed(entry({ open: false, opened_by: 'Enterprise' }))).toBe(
      'Available from the Enterprise plan.',
    )
  })

  it('still says something when the platform names no tier', () => {
    expect(whyClosed(entry({ open: false }))).toBe('Not available on your plan.')
  })
})

describe('what an offer gives', () => {
  it('reads in the units somebody came for', () => {
    expect(describeResources(entry())).toBe('500m vCPU · 1 Gi RAM · 1 Gi disk')
    expect(
      describeResources(
        entry({ resources: { cpu_millis: 2000, memory_mib: 4096, storage_gib: 20 } }),
      ),
    ).toBe('2 vCPU · 4 Gi RAM · 20 Gi disk')
  })
})

describe('where the form starts', () => {
  /**
   * Opening the page on a card the customer cannot choose makes the first
   * thing they do an error.
   */
  it('is the first offer the organisation may actually choose', () => {
    const catalogue = [
      entry({ offer: 'sandbox', open: false, opened_by: 'Starter' }),
      entry({ offer: 'standard', open: true }),
      entry({ offer: 'private', open: false, opened_by: 'Enterprise' }),
    ]

    expect(firstOpen(catalogue)).toBe('standard')
  })

  /**
   * A tier that opens nothing is a state the screen has to say out loud,
   * not present as a form with nothing selected.
   */
  it('is nothing when the tier opens nothing', () => {
    expect(firstOpen([entry({ open: false, opened_by: 'Business' })])).toBeUndefined()
    expect(firstOpen([])).toBeUndefined()
  })
})
