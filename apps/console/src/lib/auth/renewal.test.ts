import { describe, expect, it } from 'vitest'
import { RENEW_MARGIN_SECONDS, renewIn, renewalFor } from './renewal'

const NOW = 1_700_000_000

describe('renewalFor', () => {
  it('takes a session that is still good as it is', () => {
    expect(renewalFor({ expiresAt: NOW + 600, refreshToken: 'r' }, NOW)).toBe('adopt')
  })

  it('renews before the token expires, not after', () => {
    const session = { expiresAt: NOW + RENEW_MARGIN_SECONDS - 1, refreshToken: 'r' }

    expect(renewalFor(session, NOW)).toBe('exchange')
  })

  it('trades the refresh token when the access token has expired', () => {
    expect(renewalFor({ expiresAt: NOW - 3600, refreshToken: 'r' }, NOW)).toBe('exchange')
  })

  it('sends nobody to a login form while a refresh token is left', () => {
    // The symptom this whole path exists for: coming back to the tab an hour
    // later and being asked to sign in again with a valid token in hand.
    expect(renewalFor({ expiresAt: NOW - 1, refreshToken: 'r' }, NOW)).not.toBe('reauthenticate')
  })

  it('has nothing to trade without a refresh token', () => {
    expect(renewalFor({ expiresAt: NOW - 1 }, NOW)).toBe('reauthenticate')
  })

  it('has nothing to trade without a session', () => {
    expect(renewalFor(null, NOW)).toBe('reauthenticate')
  })

  it('treats a session that cannot say when it expires as expired', () => {
    expect(renewalFor({ refreshToken: 'r' }, NOW)).toBe('exchange')
  })
})

describe('renewIn', () => {
  it('waits until the margin, not until expiry', () => {
    expect(renewIn({ expiresAt: NOW + 300 }, NOW)).toBe((300 - RENEW_MARGIN_SECONDS) * 1000)
  })

  it('does not wait at all for a session already past due', () => {
    expect(renewIn({ expiresAt: NOW - 42 }, NOW)).toBe(0)
  })
})
