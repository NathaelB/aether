import { describe, expect, it } from 'vitest'
import type { User } from 'oidc-client-ts'
import { renewSession, type LockManagerLike, type Renewable } from './session'

const NOW = 1_700_000_000
const now = () => NOW

function session(expiresAt: number | undefined, refreshToken?: string): User {
  return { expires_at: expiresAt, refresh_token: refreshToken } as User
}

/** Serialises the way the Web Locks API does, without a browser. */
function serialLocks(): LockManagerLike {
  let tail: Promise<unknown> = Promise.resolve()

  return {
    request<T>(_name: string, work: () => Promise<T>): Promise<T> {
      const run = tail.then(work)
      tail = run.catch(() => undefined)

      return run
    },
  }
}

class FakeManager implements Renewable {
  exchanges = 0
  loaded: User[] = []
  events = { load: async (user: User) => void this.loaded.push(user) }

  constructor(
    private stored: User | null,
    private readonly onExchange: () => User | null = () => session(NOW + 300, 'rotated')
  ) {}

  async getUser() {
    return this.stored
  }

  async signinSilent() {
    this.exchanges += 1
    // The identity provider rotates: the successor replaces what is stored,
    // which is what a second caller must find.
    this.stored = this.onExchange()

    return this.stored
  }
}

describe('renewSession', () => {
  it('trades the refresh token when the access token has expired', async () => {
    const manager = new FakeManager(session(NOW - 10, 'r'))

    const renewed = await renewSession(manager, now, serialLocks())

    expect(manager.exchanges).toBe(1)
    expect(renewed?.refresh_token).toBe('rotated')
  })

  it('exchanges once when two tabs renew at the same moment', async () => {
    // Ferriskey revokes the whole family when one refresh token is presented
    // twice, so a second exchange would not merely be wasteful: it would sign
    // the account out of every tab, which is the bug this guards.
    const manager = new FakeManager(session(NOW - 10, 'r'))
    const locks = serialLocks()

    await Promise.all([renewSession(manager, now, locks), renewSession(manager, now, locks)])

    expect(manager.exchanges).toBe(1)
  })

  it('adopts what the other tab obtained rather than asking again', async () => {
    const manager = new FakeManager(session(NOW - 10, 'r'))
    const locks = serialLocks()

    await Promise.all([renewSession(manager, now, locks), renewSession(manager, now, locks)])

    expect(manager.loaded.map((user) => user.refresh_token)).toEqual(['rotated'])
  })

  it('reports that nothing is left to renew with', async () => {
    const manager = new FakeManager(session(NOW - 10))

    expect(await renewSession(manager, now, serialLocks())).toBeNull()
    expect(manager.exchanges).toBe(0)
  })

  it('reports that there is no session at all', async () => {
    const manager = new FakeManager(null)

    expect(await renewSession(manager, now, serialLocks())).toBeNull()
  })

  it('lets a refused exchange through to the caller', async () => {
    const manager = new FakeManager(session(NOW - 10, 'revoked'))
    manager.signinSilent = async () => {
      throw new Error('invalid_grant')
    }

    await expect(renewSession(manager, now, serialLocks())).rejects.toThrow('invalid_grant')
  })

  it('renews without the Web Locks API rather than not at all', async () => {
    const manager = new FakeManager(session(NOW - 10, 'r'))

    await renewSession(manager, now, undefined)

    expect(manager.exchanges).toBe(1)
  })
})
