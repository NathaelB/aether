import { useState } from 'react'
import {
  useGetInvitations,
  useGetMembers,
  useGetRoles,
  useInvite,
  useRemoveMember,
  useRevokeInvitation,
  useSetMemberRoles,
} from '@/api/members.api'
import { useGetUserOrganisations } from '@/api/organisation.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { PageMembers } from '../ui/page-members'

/**
 * Where the link points once somebody accepts.
 *
 * Built here rather than by the platform: the platform issues the secret and
 * knows nothing about which console will be used to walk through it.
 */
function invitationLink(token: string): string {
  return `${window.location.origin}/invitations/accept?token=${token}`
}

export default function PageMembersFeature() {
  const organisationId = useResolvedOrganisationId()
  const [link, setLink] = useState<string | null>(null)

  const members = useGetMembers(organisationId ?? null)
  const invitations = useGetInvitations(organisationId ?? null)
  const roles = useGetRoles(organisationId ?? null)
  const organisations = useGetUserOrganisations()

  const invite = useInvite()
  const revoke = useRevokeInvitation()
  const remove = useRemoveMember()
  const setRoles = useSetMemberRoles()

  const owner = organisations.data?.data?.find(
    (organisation) => organisation.id === organisationId,
  )?.owner_id

  const isSaving =
    invite.isPending || revoke.isPending || remove.isPending || setRoles.isPending

  return (
    <PageMembers
      members={members.data?.data ?? []}
      invitations={invitations.data?.data ?? []}
      roles={roles.data?.data ?? []}
      ownerId={owner}
      isLoading={members.isLoading || invitations.isLoading}
      isSaving={isSaving}
      invitationLink={link}
      onInvite={(email, granted) => {
        if (!organisationId || invite.isPending) return

        invite.mutate(
          {
            path: { organisation_id: organisationId },
            body: { email, roles: granted },
          },
          {
            // Held only for as long as the box is open. Nothing stores it,
            // and nothing can show it again.
            onSuccess: async (answer) => {
              const issued = await answer.json()
              setLink(invitationLink(issued.token))
            },
          },
        )
      }}
      onInviteDialogClosed={() => setLink(null)}
      onRevoke={(invitation) => {
        if (!organisationId || revoke.isPending) return

        revoke.mutate({
          path: { organisation_id: organisationId, invitation_id: invitation.id },
        })
      }}
      onRemove={(member) => {
        if (!organisationId || remove.isPending) return

        remove.mutate({
          path: { organisation_id: organisationId, user_id: member.user_id },
        })
      }}
      onSetRoles={(member, granted) => {
        if (!organisationId || setRoles.isPending) return

        setRoles.mutate({
          path: { organisation_id: organisationId, user_id: member.user_id },
          body: { roles: granted },
        })
      }}
    />
  )
}
