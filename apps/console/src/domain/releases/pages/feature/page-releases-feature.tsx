import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import {
  useGetReleasesForOperator,
  useMoveRelease,
  usePublishRelease,
  useWidenRollout,
} from '@/api/release.api'
import { PageReleases } from '../ui/page-releases'

export default function PageReleasesFeature() {
  const [kind, setKind] = useState<Schemas.DeploymentKind>('ferriskey')
  const releases = useGetReleasesForOperator(kind)
  const move = useMoveRelease(kind)
  const publish = usePublishRelease(kind)
  const widen = useWidenRollout(kind)

  return (
    <PageReleases
      kind={kind}
      onKindChange={setKind}
      releases={releases.data?.data ?? []}
      isLoading={releases.isLoading}
      onMove={(version, status) =>
        move.mutate({ path: { kind, version }, body: { status } })
      }
      isMoving={move.isPending}
      onPublish={(body) => publish.mutate({ path: { kind }, body })}
      isPublishing={publish.isPending}
      onOfferToEveryone={(version) =>
        widen.mutate({
          path: { kind, version },
          // Everything, with nothing held back by plan or percentage. It only
          // widens, so this is a one way gesture.
          body: { percentage: 100, plans: null, pilot_organisations: [] },
        })
      }
      isWidening={widen.isPending}
    />
  )
}
