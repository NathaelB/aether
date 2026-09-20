import { useInfiniteQuery } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

/** A screenful. The endpoint allows far more, for a caller exporting the trail. */
const PAGE = 50

/**
 * What was done to the installation, newest first.
 *
 * Paged on the keyset cursor the endpoint returns rather than on an offset,
 * which is what lets somebody keep reading while new entries are being
 * recorded: an entry recorded after a page was fetched sorts ahead of that
 * page's boundary and can never shift it.
 */
export const useGetFleetAuditLog = () => {
  const accessToken = useAuthStore(selectAccessToken)

  return useInfiniteQuery({
    queryKey: ['platform', 'audit-log'],
    enabled: !!accessToken,
    initialPageParam: undefined as string | undefined,
    // The raw client rather than the generated `queryOptions`, which builds a
    // query key from fixed parameters and so cannot carry a cursor that
    // changes per page.
    queryFn: ({ pageParam }) =>
      window.api.client.get('/platform/audit-log', {
        query: pageParam ? { cursor: pageParam, limit: PAGE } : { limit: PAGE },
      }),
    // `null` once the trail has been read to its end. Returning it as-is
    // would make TanStack ask for one more page with no cursor, which is the
    // first page again.
    getNextPageParam: (last) => last.next_cursor ?? undefined,
  })
}
