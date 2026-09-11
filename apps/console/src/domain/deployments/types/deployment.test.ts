import { describe, expect, it } from 'vitest'
import { environmentOf } from './deployment'

describe('environmentOf', () => {
  it('reads the environment off the namespace', () => {
    expect(environmentOf('production-auth')).toBe('Production')
    expect(environmentOf('development-demo')).toBe('Development')
  })

  it('keeps the rest of a name that has its own dashes', () => {
    expect(environmentOf('staging-auth-eu-west')).toBe('Staging')
  })

  /**
   * A namespace that does not follow the shape is shown whole. Cutting at a
   * separator that is not there would present half a name as an environment.
   */
  it('shows a namespace it cannot read whole', () => {
    expect(environmentOf('auth')).toBe('auth')
    expect(environmentOf('-auth')).toBe('-auth')
  })
})
