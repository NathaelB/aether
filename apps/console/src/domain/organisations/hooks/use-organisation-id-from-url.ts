import { useMemo } from 'react'
import { useRouterState } from '@tanstack/react-router'

export const useOrganisationIdFromUrl = () => {
  const { location } = useRouterState()

  return useMemo(() => {
    const match = location.pathname.match(/^\/organisations\/([^/]+)(?:\/|$)/)
    if (!match) {
      return null
    }

    const candidate = decodeURIComponent(match[1])
    if (candidate === 'create') {
      return null
    }

    return candidate
  }, [location.pathname])
}
