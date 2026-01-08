import { create } from 'zustand'
import { devtools } from 'zustand/middleware'
import type { IdTokenClaims, User } from 'oidc-client-ts'

interface AuthState {
  user: User | null
  profile: IdTokenClaims | null
  accessToken: string | null
  isAuthenticated: boolean

  setUser: (user: User | null) => void
  setProfile: (profile: IdTokenClaims | null) => void
  setAccessToken: (token: string | null) => void
  clear: () => void

  getAccessToken: () => string | null
}

export const useAuthStore = create<AuthState>()(
  devtools(
    (set, get) => ({
      user: null,
      accessToken: null,
      isAuthenticated: false,

      setUser: (user) =>
        set({
          user,
          accessToken: user?.access_token || null,
          isAuthenticated: !!user,
        }),

      setProfile: (profile) =>
        set({
          profile,
        }),

      setAccessToken: (token) =>
        set({
          accessToken: token,
        }),

      clear: () =>
        set({
          user: null,
          accessToken: null,
          isAuthenticated: false,
        }),

      getAccessToken: () => get().accessToken,
    }),
    { name: 'auth-store' }
  )
)

// Selector pour éviter les re-renders inutiles
export const selectAccessToken = (state: AuthState) => state.accessToken
export const selectIsAuthenticated = (state: AuthState) => state.isAuthenticated
export const selectUser = (state: AuthState) => state.user
export const selectProfile = (state: AuthState) => state.profile
