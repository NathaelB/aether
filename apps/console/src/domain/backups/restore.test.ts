import { describe, expect, it } from 'vitest'
import { checkRestoreName } from './restore'

describe('a restore name', () => {
  it('is refused when empty', () => {
    expect(checkRestoreName('')).toContain('name of its own')
  })

  it('is refused when only whitespace', () => {
    expect(checkRestoreName('   ')).toContain('name of its own')
  })

  it('is accepted otherwise', () => {
    expect(checkRestoreName('acme-recovery')).toBeNull()
  })
})
