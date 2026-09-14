import { useState } from 'react'
import { useParams } from '@tanstack/react-router'
import type { Schemas } from '@/api/api.client'
import { useAskForBackup, useGetEstateDeployments, useGetTenant } from '@/api/platform.api'
import { PageTenantDetail } from '../ui/page-tenant-detail'

export default function PageTenantDetailFeature() {
  const { organisationId } = useParams({ strict: false }) as { organisationId: string }
  const [outcome, setOutcome] = useState<{ ok: boolean; message: string }>()

  const tenant = useGetTenant(organisationId)
  const deployments = useGetEstateDeployments({ organisationId })
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
        onSuccess: () =>
          setOutcome({
            ok: true,
            message: `${row.deployment.name}: the data plane has been told. The archive appears in that deployment's backups once it reports one.`,
          }),
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
    <PageTenantDetail
      tenant={tenant.data?.data}
      deployments={deployments.data?.data ?? []}
      // The organisation's own facts are what this page is; its deployments
      // fill in beside them. Waiting for both would leave the page blank on
      // an installation whose estate query is the slower of the two.
      isLoading={tenant.isLoading}
      asking={ask.isPending ? ask.variables?.path.deployment_id : undefined}
      onAskForBackup={askFor}
      outcome={outcome}
    />
  )
}
