import { useNavigate } from '@tanstack/react-router'
import { useMemo } from 'react'
import PageCreateDeployment from '../ui/page-create-deployment'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { useCreateDeployment } from '@/api/deployment.api'
import { useGetDataplanes } from '@/api/dataplane.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { servedRegions } from '@/domain/dataplanes/served-regions'
import {
  toCreateDeploymentRequest,
  type CreateDeploymentForm,
} from '../../create-deployment-request'

export default function PageCreateDeploymentFeature() {
  const navigate = useNavigate()
  const organisationPath = useOrganisationPath()
  const organisationId = useResolvedOrganisationId()
  const createDeployment = useCreateDeployment()
  const dataplanes = useGetDataplanes()

  const regions = useMemo(
    () => servedRegions(dataplanes.data?.data ?? []),
    [dataplanes.data],
  )

  // `capacity` is collected by the form and has no field on the API. It gates
  // which plans are offered, so it is not dead -- but it is not sent either,
  // and pretending otherwise here would repeat the bug this page just fixed.
  const handleCreate = async (data: CreateDeploymentForm & { capacity: number }) => {
    if (!organisationId) {
      return
    }

    createDeployment.mutate(
      {
        path: { organisation_id: organisationId },
        body: toCreateDeploymentRequest(data),
      },
      {
        onSuccess: () => {
          navigate({ to: organisationPath('/deployments') })
        },
      },
    )
  }

  return (
    <PageCreateDeployment
      onSubmit={handleCreate}
      isSubmitting={createDeployment.isPending}
      regions={regions}
      regionsLoading={dataplanes.isLoading}
    />
  )
}
