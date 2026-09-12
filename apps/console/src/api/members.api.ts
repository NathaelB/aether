import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

function membersKey(organisationId: string) {
  return window.api.get('/organisations/{organisation_id}/members', {
    path: { organisation_id: organisationId },
  }).queryKey
}

function invitationsKey(organisationId: string) {
  return window.api.get('/organisations/{organisation_id}/invitations', {
    path: { organisation_id: organisationId },
  }).queryKey
}

export const useGetMembers = (organisationId: string | null) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/members', {
      path: { organisation_id: organisationId ?? 'current' },
    }).queryOptions,
    enabled: !!organisationId && !!accessToken,
  })
}

export const useGetInvitations = (organisationId: string | null) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/invitations', {
      path: { organisation_id: organisationId ?? 'current' },
    }).queryOptions,
    enabled: !!organisationId && !!accessToken,
  })
}

export const useGetRoles = (organisationId: string | null) => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/organisations/{organisation_id}/roles', {
      path: { organisation_id: organisationId ?? 'current' },
    }).queryOptions,
    enabled: !!organisationId && !!accessToken,
  })
}

/**
 * Both lists, after anything that changes either.
 *
 * Accepting an invitation makes a member and closes the invitation, so a
 * screen refreshing only one of them shows two answers to the same question.
 */
function refreshBoth(queryClient: ReturnType<typeof useQueryClient>, organisationId: string) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: membersKey(organisationId) }),
    queryClient.invalidateQueries({ queryKey: invitationsKey(organisationId) }),
  ])
}

export const useSetMemberRoles = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation('put', '/organisations/{organisation_id}/members/{user_id}/roles')
      .mutationOptions,
    onSuccess: async (_, variables) => {
      await refreshBoth(queryClient, variables.path.organisation_id)
    },
  })
}

export const useRemoveMember = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation('delete', '/organisations/{organisation_id}/members/{user_id}')
      .mutationOptions,
    onSuccess: async (_, variables) => {
      await refreshBoth(queryClient, variables.path.organisation_id)
    },
  })
}

export const useInvite = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation('post', '/organisations/{organisation_id}/invitations').mutationOptions,
    onSuccess: async (_, variables) => {
      await refreshBoth(queryClient, variables.path.organisation_id)
    },
  })
}

export const useRevokeInvitation = () => {
  const queryClient = useQueryClient()

  return useMutation({
    ...window.api.mutation(
      'delete',
      '/organisations/{organisation_id}/invitations/{invitation_id}',
    ).mutationOptions,
    onSuccess: async (_, variables) => {
      await refreshBoth(queryClient, variables.path.organisation_id)
    },
  })
}

/**
 * Walks somebody through their invitation.
 *
 * Outside any organisation: whoever is accepting is not in one yet, so there
 * is nothing to scope the call to and nothing to invalidate by id. The caller
 * clears the cache wholesale afterwards, because what they may see has just
 * changed everywhere.
 */
export const useAcceptInvitation = () => {
  return useMutation({
    ...window.api.mutation('post', '/invitations/accept').mutationOptions,
  })
}
