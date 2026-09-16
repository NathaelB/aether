import { useState } from 'react'
import { useParams } from '@tanstack/react-router'
import type { Schemas } from '@/api/api.client'
import {
  useAskForBackup,
  useGetEstateDeployments,
  useGetTenant,
  useMoveTenantPlan,
} from '@/api/platform.api'
import { useHoldsPlatformRight } from '@/domain/organisations/hooks/use-is-operator'
import { PageTenantDetail } from '../ui/page-tenant-detail'

export default function PageTenantDetailFeature() {
  const { organisationId } = useParams({ strict: false }) as { organisationId: string }
  const [outcome, setOutcome] = useState<{ ok: boolean; message: string }>()

  const tenant = useGetTenant(organisationId)
  const deployments = useGetEstateDeployments({ organisationId })
  const ask = useAskForBackup()
  const move = useMoveTenantPlan()
  const canAct = useHoldsPlatformRight('act_on_tenant')

  const moveToPlan = (plan: string) => {
    setOutcome(undefined)

    move.mutate(
      { path: { organisation_id: organisationId }, body: { plan } },
      {
        onSuccess: () => setOutcome({ ok: true, message: `Moved to ${plan}.` }),
        onError: (error: unknown) =>
          setOutcome({
            ok: false,
            // The refusal is worth repeating rather than replacing: it names
            // what is in the way, which is what the operator has to act on.
            message: error instanceof Error ? error.message : 'The plan could not be changed.',
          }),
      },
    )
  }

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
      canAct={canAct}
      onMoveToPlan={moveToPlan}
      moving={move.isPending}
    />
  )
}
