import { useEffect } from 'react'
import { useGetUserOrganisations } from '@/api/organisation.api'
import { useOrganisationsStore } from '@/stores/organisations'

export const useUserOrganisations = () => {
  const setOrganisations = useOrganisationsStore((state) => state.setOrganisations)
  const setLoaded = useOrganisationsStore((state) => state.setLoaded)

  const query = useGetUserOrganisations()

  useEffect(() => {
    if (query.status === 'success') {
      setOrganisations(query.data?.data ?? [])
    }
  }, [query.status, query.data, setOrganisations])

  useEffect(() => {
    if (query.status === 'error') {
      setLoaded(true)
    }
  }, [query.status, setLoaded])

  return query
}
