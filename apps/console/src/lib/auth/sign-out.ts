import { useAuthStore } from '@/stores/auth'
import { getUserManager } from './user-manager'

/**
 * Ends the session, everywhere it exists.
 *
 * Clearing what the console holds in memory used to be the whole of it, which
 * left the refresh token where it was -- and now that it outlives the tab,
 * that would be a signed-out browser still carrying a working credential.
 * The token is revoked first, because that is the part no later step can
 * undo, and the local session goes even when the identity provider refuses.
 */
export async function signOut(): Promise<void> {
  const manager = getUserManager()

  useAuthStore.getState().clear()

  if (!manager) {
    window.location.assign('/')
    return
  }

  try {
    // Revokes the tokens and drops the stored user on the way out.
    await manager.signoutRedirect()
  } catch {
    await manager.removeUser()
    window.location.assign('/')
  }
}
