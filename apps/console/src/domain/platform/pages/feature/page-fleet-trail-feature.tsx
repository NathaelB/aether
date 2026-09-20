import { useGetFleetAuditLog } from '@/api/fleet-audit.api'
import { PageFleetTrail } from '../ui/page-fleet-trail'

export default function PageFleetTrailFeature() {
  const trail = useGetFleetAuditLog()

  return (
    <PageFleetTrail
      entries={trail.data?.pages.flatMap((page) => page.data) ?? []}
      isLoading={trail.isLoading}
      hasMore={trail.hasNextPage}
      isLoadingMore={trail.isFetchingNextPage}
      onLoadMore={() => trail.fetchNextPage()}
    />
  )
}
