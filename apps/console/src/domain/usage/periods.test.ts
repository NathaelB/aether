import { afterEach, describe, expect, it, vi } from 'vitest'
import { PERIODS, periodFor, rememberPeriod, rememberedPeriod } from './periods'

function inMemoryStorage() {
  const held = new Map<string, string>()

  return {
    getItem: (key: string) => held.get(key) ?? null,
    setItem: (key: string, value: string) => void held.set(key, value),
  }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('periodFor', () => {
  it('reads back every period it offers', () => {
    for (const period of PERIODS) {
      expect(periodFor(period.key).key).toBe(period.key)
    }
  })

  /** A stored value from an older build, or a hand-edited one. */
  it('falls back to a day for anything it does not recognise', () => {
    expect(periodFor('a fortnight').key).toBe('24h')
    expect(periodFor(null).key).toBe('24h')
  })
})

describe('remembering the choice', () => {
  it('survives a reload', () => {
    vi.stubGlobal('localStorage', inMemoryStorage())

    rememberPeriod('7d')

    expect(rememberedPeriod().key).toBe('7d')
  })

  /**
   * A browser with site data blocked throws on the access itself. A dashboard
   * that cannot render in a private window is worse than one that forgets.
   */
  it('still answers when storage refuses to be touched', () => {
    vi.stubGlobal('localStorage', {
      getItem: () => {
        throw new Error('blocked')
      },
      setItem: () => {
        throw new Error('blocked')
      },
    })

    expect(() => rememberPeriod('1h')).not.toThrow()
    expect(rememberedPeriod().key).toBe('24h')
  })
})
