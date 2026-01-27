import { create } from 'zustand'
import { devtools, persist } from 'zustand/middleware'
import type { Organisation } from '@/api/api.client'

interface OrganisationsState {
  organisations: Organisation[]
  hasLoaded: boolean
  activeOrganisationId: string | null
  setOrganisations: (organisations: Organisation[]) => void
  setLoaded: (loaded: boolean) => void
  setActiveOrganisationId: (organisationId: string | null) => void
  clear: () => void
}

export const useOrganisationsStore = create<OrganisationsState>()(
  devtools(
    persist(
      (set) => ({
        organisations: [],
        hasLoaded: false,
        activeOrganisationId: null,
        setOrganisations: (organisations) =>
          set({
            organisations,
            hasLoaded: true,
          }),
        setLoaded: (loaded) =>
          set({
            hasLoaded: loaded,
          }),
        setActiveOrganisationId: (organisationId) =>
          set({
            activeOrganisationId: organisationId,
          }),
        clear: () =>
          set({
            organisations: [],
            hasLoaded: false,
            activeOrganisationId: null,
          }),
      }),
      {
        name: 'organisations-store',
        partialize: (state) => ({
          activeOrganisationId: state.activeOrganisationId,
        }),
      }
    ),
    { name: 'organisations-store' }
  )
)

export const selectOrganisations = (state: OrganisationsState) => state.organisations
export const selectOrganisationsLoaded = (state: OrganisationsState) => state.hasLoaded
export const selectActiveOrganisationId = (state: OrganisationsState) =>
  state.activeOrganisationId
