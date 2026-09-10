import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import { useGetReleasesForOperator, useMoveRelease } from '@/api/release.api'
import { PageReleases } from '../ui/page-releases'

export default function PageReleasesFeature() {
  const [kind, setKind] = useState<Schemas.DeploymentKind>('ferriskey')
  const releases = useGetReleasesForOperator(kind)
  const move = useMoveRelease(kind)

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
    />
  )
}
