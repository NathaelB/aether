import { useEffect, useRef, useState } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { useQueryClient } from '@tanstack/react-query'
import { useAcceptInvitation } from '@/api/members.api'
import { organisationPathFor } from '@/lib/paths'
import { useOrganisationsStore } from '@/stores/organisations'
import { PageAcceptInvitation, type AcceptOutcome } from '../ui/page-accept-invitation'
import { tokenFromLink } from '../../members'

export default function PageAcceptInvitationFeature() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const accept = useAcceptInvitation()
  const setActive = useOrganisationsStore((state) => state.setActiveOrganisationId)

  const token = tokenFromLink(window.location.search)
  const [outcome, setOutcome] = useState<AcceptOutcome>(
    token ? { kind: 'working' } : { kind: 'no-token' },
  )

  // Once, whatever React does with the effect. Walking through an invitation
  // is not a read: the second attempt would answer "already accepted" and
  // turn a success into a failure on screen.
  const attempted = useRef(false)

  useEffect(() => {
    if (!token || attempted.current) return
    attempted.current = true

    // Taken out of the address bar before anything else. It is a credential,
    // and leaving it there puts it in history, in whatever the page loads
    // next, and in whatever somebody pastes when they share the tab.
    window.history.replaceState({}, '', window.location.pathname)

    accept.mutate(
      { body: { token } },
      {
        onSuccess: async (answer: Response) => {
          const member = (await answer.json()).data
          setActive(member.organisation_id)
          await queryClient.invalidateQueries()
          setOutcome({ kind: 'joined', organisation: member.organisation_id })
        },
        onError: (error: unknown) =>
          setOutcome({ kind: 'refused', reason: readReason(error) }),
      },
    )
    // Deliberately keyed on the token alone: the mutation and the navigator
    // change identity between renders and would re-run this.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token])

  return (
    <PageAcceptInvitation
      outcome={outcome}
      onOpen={(organisation) => navigate({ to: organisationPathFor(organisation) })}
      onGoHome={() => navigate({ to: '/' })}
    />
  )
}

/**
 * The platform's own sentence, when it sent one.
 *
 * It already tells expired from unknown from addressed-to-somebody-else, and
 * a generic message here would throw that away.
 */
function readReason(error: unknown): string {
  const message = (error as { message?: string } | null)?.message

  return message && message.length > 0
    ? message
    : 'The invitation could not be used. Ask whoever invited you for another link.'
}
