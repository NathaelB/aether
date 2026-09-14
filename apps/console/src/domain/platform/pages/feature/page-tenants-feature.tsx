import { useNavigate } from '@tanstack/react-router'
import { useGetTenants } from '@/api/platform.api'
import { platformPath } from '@/lib/paths'
import { PageTenants } from '../ui/page-tenants'

export default function PageTenantsFeature() {
  const tenants = useGetTenants()
  const navigate = useNavigate()

  return (
    <PageTenants
      tenants={tenants.data?.data ?? []}
      isLoading={tenants.isLoading}
      // "What does this tenant have" is the question asked right after "who
      // is on this installation", and it is answered by the other screen
      // rather than by a third one that lists the same rows again.
      onShowDeployments={(organisationId) =>
        navigate({
          to: platformPath('/deployments'),
          search: { organisation_id: organisationId },
        })
      }
    />
  )
}
