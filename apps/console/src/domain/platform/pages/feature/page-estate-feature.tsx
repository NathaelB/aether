import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import { useAskForBackup, useGetEstateDeployments } from '@/api/platform.api'
import type { EstateFilters } from '../../estate-filters'
import { WHOLE_ESTATE } from '../../estate-filters'
import { PageEstate } from '../ui/page-estate'

export default function PageEstateFeature() {
  const [filters, setFilters] = useState<EstateFilters>(WHOLE_ESTATE)
  const [outcome, setOutcome] = useState<{ ok: boolean; message: string }>()

  const estate = useGetEstateDeployments(filters)
  const ask = useAskForBackup()

  const askFor = (row: Schemas.EstateDeployment) => {
    setOutcome(undefined)

    ask.mutate(
      {
        path: {
          organisation_id: row.organisation.id,
          deployment_id: row.deployment.id,
        },
      },
      {
        // Said plainly, including where to go and look. The archive does not
        // exist yet and nothing on this page will change when it does.
        onSuccess: () =>
          setOutcome({
            ok: true,
            message: `${row.organisation.name} / ${row.deployment.name}: the data plane has been told. The archive appears in that deployment's backups once it reports one.`,
          }),
        // The platform's own words. It refuses for reasons worth reading —
        // an archive already on its way, an installation that archives
        // nowhere — and a generic "something went wrong" would hide both.
        onError: (error: unknown) =>
          setOutcome({
            ok: false,
            message:
              error instanceof Error ? error.message : 'The archive could not be asked for.',
          }),
      },
    )
  }

  return (
    <PageEstate
      deployments={estate.data?.data ?? []}
      isLoading={estate.isLoading}
      filters={filters}
      onFilter={setFilters}
      asking={ask.isPending ? ask.variables?.path.deployment_id : undefined}
      onAskForBackup={askFor}
      outcome={outcome}
    />
  )
}
