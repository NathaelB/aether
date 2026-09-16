import { useState } from 'react'
import { useCreateDataplane, useGetDataplanes } from '@/api/dataplane.api'
import { useGetTenants } from '@/api/platform.api'
import { useHoldsPlatformRight } from '@/domain/organisations/hooks/use-is-operator'
import { PageDataPlanes } from '../ui/page-dataplanes'
import { RegisterDataPlaneSheet } from '../ui/register-dataplane-sheet'

export default function PageDataPlanesFeature() {
  const dataplanes = useGetDataplanes()
  const canOperate = useHoldsPlatformRight('operate_fleet')

  const [registering, setRegistering] = useState(false)
  const create = useCreateDataplane()

  // Only fetched behind the sheet, and only for the dedicated case: the list
  // of every tenant is not something the fleet screen needs to draw itself.
  const organisations = useGetTenants()

  const answered = create.data
  const registered =
    answered?.herald_client_id && answered.herald_secret
      ? {
          dataplaneId: answered.id,
          clientId: answered.herald_client_id,
          clientSecret: answered.herald_secret,
        }
      : undefined

  return (
    <>
      <PageDataPlanes
        dataplanes={dataplanes.data?.data ?? []}
        isLoading={dataplanes.isLoading}
        canOperate={canOperate}
        onRegister={() => {
          // The previous answer holds a secret. Cleared on the way in rather
          // than on the way out, so reopening never shows a credential
          // belonging to the cluster registered before this one.
          create.reset()
          setRegistering(true)
        }}
      />

      <RegisterDataPlaneSheet
        open={registering}
        onOpenChange={setRegistering}
        organisations={organisations.data?.data ?? []}
        onRegister={(body) => create.mutate({ body })}
        isRegistering={create.isPending}
        registered={registered}
        refusal={create.error instanceof Error ? create.error.message : undefined}
        apiUrl={window.apiUrl}
        issuerUrl={window.issuerUrl ?? ''}
      />
    </>
  )
}
