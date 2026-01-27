import { create } from 'zustand'
import { devtools } from 'zustand/middleware'
import type { Organisation } from '@/api/api.client'

interface OrganisationsState {
  organisations: Organisation[]
  hasLoaded: boolean
  setOrganisations: (organisations: Organisation[]) => void
  setLoaded: (loaded: boolean) => void
  clear: () => void
}

export const useOrganisationsStore = create<OrganisationsState>()(
  devtools(
    (set) => ({
      organisations: [],
      hasLoaded: false,
      setOrganisations: (organisations) =>
        set({
          organisations,
          hasLoaded: true,
        }),
      setLoaded: (loaded) =>
        set({
          hasLoaded: loaded,
        }),
      clear: () =>
        set({
          organisations: [],
          hasLoaded: false,
        }),
    }),
    { name: 'organisations-store' }
  )
)

export const selectOrganisations = (state: OrganisationsState) => state.organisations
export const selectOrganisationsLoaded = (state: OrganisationsState) => state.hasLoaded
