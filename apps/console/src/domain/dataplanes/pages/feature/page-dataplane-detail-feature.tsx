import { useParams } from '@tanstack/react-router'
import type { Schemas } from '@/api/api.client'
import {
  useGetDataplane,
  useGetDataplaneDeployments,
  useReissueHeraldCredential,
  useSetDataplaneService,
} from '@/api/dataplane.api'
import { useHoldsPlatformRight } from '@/domain/organisations/hooks/use-is-operator'
import { PageDataPlaneDetail } from '../ui/page-dataplane-detail'

export default function PageDataPlaneDetailFeature() {
  const { dataplaneId } = useParams({ strict: false }) as { dataplaneId?: string }
  const dataplane = useGetDataplane(dataplaneId ?? null)
  const deployments = useGetDataplaneDeployments(dataplaneId ?? null)

  const setService = useSetDataplaneService()
  const reissue = useReissueHeraldCredential()
  const canOperate = useHoldsPlatformRight('operate_fleet')

  const issued = reissue.data
  const reissued =
    issued?.herald_client_id && issued.herald_secret
      ? { clientId: issued.herald_client_id, clientSecret: issued.herald_secret }
      : undefined

  return (
    <PageDataPlaneDetail
      dataplane={dataplane.data?.data}
      deployments={deployments.data?.data ?? []}
      isLoading={dataplane.isLoading}
      canOperate={canOperate}
      onSetService={(service: Schemas.ServiceIntent) => {
        if (!dataplaneId) return

        setService.mutate({ path: { dataplane_id: dataplaneId }, body: { service } })
      }}
      pending={setService.isPending ? setService.variables?.body.service : undefined}
      // Repeated rather than replaced with something friendlier: the control
      // plane's refusal names the cluster's own state, which is the thing the
      // operator has to act on.
      refusal={
        setService.error instanceof Error
          ? setService.error.message
          : reissue.error instanceof Error
            ? reissue.error.message
            : undefined
      }
      onReissue={() => {
        if (!dataplaneId) return

        reissue.mutate({ path: { dataplane_id: dataplaneId } })
      }}
      isReissuing={reissue.isPending}
      reissued={reissued}
      // Dropped from the mutation's cache, not merely hidden: it is the only
      // copy of a secret, and a screen that kept it would hand it back to
      // whoever reopened the page.
      onCredentialDismissed={() => reissue.reset()}
      apiUrl={window.apiUrl}
      issuerUrl={window.issuerUrl ?? ''}
    />
  )
}
