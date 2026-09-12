import { describe, expect, it } from 'vitest'
import { ApiRequestError } from './api.fetch'

describe('ApiRequestError', () => {
  /**
   * The symptom this exists for: a screen somebody may not open spent four
   * requests being told no before drawing anything, which reads as a slow
   * page rather than a closed door.
   */
  it('does not retry what the platform refused', () => {
    for (const status of [400, 401, 403, 404, 409, 422]) {
      expect(new ApiRequestError(status, 'no').worthRetrying, String(status)).toBe(false)
    }
  })

  /** Both say "not now" rather than "not this". */
  it('retries the two that mean wait', () => {
    expect(new ApiRequestError(408, 'timeout').worthRetrying).toBe(true)
    expect(new ApiRequestError(429, 'slow down').worthRetrying).toBe(true)
  })

  it('retries what broke on the far side', () => {
    for (const status of [500, 502, 503, 504]) {
      expect(new ApiRequestError(status, 'oops').worthRetrying, String(status)).toBe(true)
    }
  })

  /**
   * The platform's sentence, not a rebuilt one. It already tells an expired
   * invitation from an unknown one and from one addressed to somebody else.
   */
  it('keeps what the platform said', async () => {
    const refused = await ApiRequestError.from(
      new Response(JSON.stringify({ code: 'E_CONFLICT', message: 'this invitation expired' }), {
        status: 409,
      }),
    )

    expect(refused.status).toBe(409)
    expect(refused.message).toBe('this invitation expired')
    expect(refused.code).toBe('E_CONFLICT')
  })

  it('falls back to the status line when there is no message to read', async () => {
    const refused = await ApiRequestError.from(
      new Response('<html>gateway</html>', { status: 502, statusText: 'Bad Gateway' }),
    )

    expect(refused.status).toBe(502)
    expect(refused.message).toContain('502')
  })
})
