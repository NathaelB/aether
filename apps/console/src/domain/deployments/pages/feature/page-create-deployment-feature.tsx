import { useNavigate } from '@tanstack/react-router'
import PageCreateDeployment from '../ui/page-create-deployment'
import { DeploymentType, Environment, DeploymentPlan } from '../../types/deployment'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { useCreateDeployment } from '@/api/deployment.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'

const slugify = (value: string) =>
  value
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/(^-|-$)+/g, '')

export default function PageCreateDeploymentFeature() {
  const navigate = useNavigate()
  const organisationPath = useOrganisationPath()
  const organisationId = useResolvedOrganisationId()
  const createDeployment = useCreateDeployment()

  const kindByType: Record<DeploymentType, 'keycloak' | 'ferriskey'> = {
    keycloak: 'keycloak',
    ferriskey: 'ferriskey',
    authentik: 'keycloak',
  }

  const handleCreate = async (data: { 
    name: string; 
    type: DeploymentType; 
    environment: Environment;
    region: string;
    plan: DeploymentPlan;
    capacity: number;
  }) => {
    if (!organisationId) {
      return
    }

    const namespace = slugify(`${data.environment}-${data.name}`)

    createDeployment.mutate(
      {
        path: { organisation_id: organisationId },
        body: {
          name: data.name,
          kind: kindByType[data.type],
          namespace,
          version: 'latest',
        },
      },
      {
        onSuccess: () => {
          navigate({ to: organisationPath('/deployments') })
        },
      }
    )
  }

  return (
    <PageCreateDeployment
      onSubmit={handleCreate}
      isSubmitting={createDeployment.isPending}
    />
  )
}
