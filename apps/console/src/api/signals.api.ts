import { useQuery } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

/**
 * Open signals, newest first.
 *
 * A single page rather than the infinite-scroll shape the trail uses: this
 * is read for an at-a-glance count and a short recent list, not for paging
 * back through everything a probe has ever reported.
 */
export const useGetOpenSignals = (limit = 20) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/platform/signals', { query: { limit } }).queryOptions,
    enabled: !!accessToken,
  })
}
