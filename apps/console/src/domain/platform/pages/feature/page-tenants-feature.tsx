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
      // The organisation itself, not a list filtered to it. What a tenant is
      // -- its plan, what it is allowed, how close to that it runs -- is the
      // question somebody clicks a name to answer; what it runs is on that
      // same page, under it.
      onOpen={(organisationId) =>
        navigate({ to: platformPath(`/organisations/${organisationId}`) })
      }
    />
  )
}
