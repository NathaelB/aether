import {
  useCreateRole,
  useDeleteRole,
  useGetMembers,
  useGetRoles,
  useUpdateRole,
} from '@/api/members.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { PageRoles } from '../ui/page-roles'

export default function PageRolesFeature() {
  const organisationId = useResolvedOrganisationId()

  const roles = useGetRoles(organisationId ?? null)
  // Read to count who holds what. A role's row says how many people it
  // affects, which is what makes deleting one a decision rather than a leap.
  const members = useGetMembers(organisationId ?? null)

  const create = useCreateRole()
  const update = useUpdateRole()
  const remove = useDeleteRole()

  const isSaving = create.isPending || update.isPending || remove.isPending

  return (
    <PageRoles
      roles={roles.data?.data ?? []}
      members={members.data?.data ?? []}
      isLoading={roles.isLoading}
      isSaving={isSaving}
      onCreate={(name, permissions) => {
        if (!organisationId || create.isPending) return

        create.mutate({
          path: { organisation_id: organisationId },
          body: { name, permissions, color: null },
        })
      }}
      onUpdate={(role, name, permissions) => {
        if (!organisationId || update.isPending) return

        update.mutate({
          path: { organisation_id: organisationId, role_id: role.id },
          body: { name, permissions },
        })
      }}
      onDelete={(role) => {
        if (!organisationId || remove.isPending) return

        remove.mutate({
          path: { organisation_id: organisationId, role_id: role.id },
        })
      }}
    />
  )
}
