import { useState } from 'react'
import { Crown, MailPlus, Trash2, UserPlus, X } from 'lucide-react'
import { EmptyState, Section, SettingsPage } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { StatusBadge } from '@/components/ui/status-badge'
import { cn } from '@/lib/utils'
import {
  describeRoles,
  expiresIn,
  isOwner,
  outstanding,
  type Invitation,
  type Member,
  type Role,
} from '../../members'
import { InviteDialog } from './invite-dialog'

interface Props {
  members: Member[]
  invitations: Invitation[]
  roles: Role[]
  ownerId?: string
  isLoading: boolean
  isSaving: boolean
  invitationLink: string | null
  onInvite: (email: string, roles: string[]) => void
  onInviteDialogClosed: () => void
  onRevoke: (invitation: Invitation) => void
  onRemove: (member: Member) => void
  onSetRoles: (member: Member, roles: string[]) => void
}

export function PageMembers({
  members,
  invitations,
  roles,
  ownerId,
  isLoading,
  isSaving,
  invitationLink,
  onInvite,
  onInviteDialogClosed,
  onRevoke,
  onRemove,
  onSetRoles,
}: Props) {
  const [inviting, setInviting] = useState(false)
  const [removing, setRemoving] = useState<Member | null>(null)
  const [editing, setEditing] = useState<Member | null>(null)

  if (isLoading) {
    return <Skeleton className='h-64 w-full' />
  }

  const now = new Date()
  const waiting = outstanding(invitations, now)

  return (
    <SettingsPage title='Members' description='Who is in this organisation, and what they may do.'>
      <div className='space-y-8'>
        <Section
          title='Members'
          aside={
            <Button size='sm' onClick={() => setInviting(true)} disabled={isSaving}>
              <UserPlus className='h-3.5 w-3.5' />
              Invite somebody
            </Button>
          }
        >
          <div className='overflow-hidden rounded-lg border'>
            <ul>
              {members.map((member) => {
                const owner = isOwner(member, ownerId)

                return (
                  <li
                    key={member.id}
                    className='flex items-center justify-between gap-3 border-b px-4 py-3 last:border-b-0'
                  >
                    <div className='min-w-0'>
                      <div className='flex items-center gap-2'>
                        <span className='truncate text-sm font-medium'>{member.name}</span>
                        {owner && (
                          <StatusBadge tone='accent' dot={false}>
                            <Crown className='mr-1 h-3 w-3' />
                            Owner
                          </StatusBadge>
                        )}
                      </div>
                      <p className='truncate text-xs text-muted-foreground'>
                        {member.email} — {describeRoles(member, ownerId)}
                      </p>
                    </div>

                    <div className='flex shrink-0 items-center gap-2'>
                      <Button
                        variant='outline'
                        size='sm'
                        disabled={isSaving || owner}
                        title={
                          owner
                            ? 'The owner holds everything through the organisation, not a role'
                            : 'Change what they may do'
                        }
                        onClick={() => setEditing(member)}
                      >
                        Roles
                      </Button>
                      <Button
                        variant='outline'
                        size='icon'
                        aria-label={`Remove ${member.name}`}
                        disabled={isSaving || owner}
                        title={
                          owner
                            ? 'The owner cannot be removed: an organisation without one is unrecoverable'
                            : `Remove ${member.name}`
                        }
                        onClick={() => setRemoving(member)}
                      >
                        <Trash2 className='h-4 w-4' />
                      </Button>
                    </div>
                  </li>
                )
              })}
            </ul>
          </div>
        </Section>

        <Section title='Waiting to join'>
          {waiting.length === 0 ? (
            <EmptyState
              icon={<MailPlus className='h-5 w-5' />}
              title='Nobody is waiting'
              description='An invitation appears here until it is used, revoked, or lapses.'
            />
          ) : (
            <div className='overflow-hidden rounded-lg border'>
              <ul>
                {waiting.map((invitation) => (
                  <li
                    key={invitation.id}
                    className='flex items-center justify-between gap-3 border-b px-4 py-3 last:border-b-0'
                  >
                    <div className='min-w-0'>
                      <p className='truncate text-sm'>{invitation.email}</p>
                      <p className='truncate text-xs text-muted-foreground'>
                        {expiresIn(invitation, now)}
                        {invitation.roles.length > 0 &&
                          ` — will hold ${invitation.roles.map((role) => role.name).join(', ')}`}
                      </p>
                    </div>
                    <Button
                      variant='outline'
                      size='icon'
                      aria-label={`Revoke the invitation for ${invitation.email}`}
                      disabled={isSaving}
                      onClick={() => onRevoke(invitation)}
                    >
                      <X className='h-4 w-4' />
                    </Button>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </Section>
      </div>

      <InviteDialog
        open={inviting}
        onOpenChange={(open) => {
          setInviting(open)
          if (!open) onInviteDialogClosed()
        }}
        members={members}
        invitations={invitations}
        roles={roles}
        isSaving={isSaving}
        onInvite={onInvite}
        link={invitationLink}
      />

      <ConfirmRemoval
        member={removing}
        isSaving={isSaving}
        onCancel={() => setRemoving(null)}
        onConfirm={(member) => {
          onRemove(member)
          setRemoving(null)
        }}
      />

      <EditRoles
        member={editing}
        roles={roles}
        isSaving={isSaving}
        onCancel={() => setEditing(null)}
        onSave={(member, granted) => {
          onSetRoles(member, granted)
          setEditing(null)
        }}
      />
    </SettingsPage>
  )
}

/**
 * Asked first, because it is not reversible from this screen: getting back in
 * needs another invitation somebody has to send.
 */
function ConfirmRemoval({
  member,
  isSaving,
  onCancel,
  onConfirm,
}: {
  member: Member | null
  isSaving: boolean
  onCancel: () => void
  onConfirm: (member: Member) => void
}) {
  return (
    <Dialog open={!!member} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent>
        {member && (
          <>
            <DialogHeader>
              <DialogTitle>Remove {member.name}?</DialogTitle>
              <DialogDescription>
                They lose everything they hold here immediately. Getting back in needs another
                invitation, which somebody has to send.
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button variant='outline' onClick={onCancel}>
                Cancel
              </Button>
              <Button disabled={isSaving} onClick={() => onConfirm(member)}>
                {isSaving && <Spinner className='size-3.5' />}
                Remove
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}

function EditRoles({
  member,
  roles,
  isSaving,
  onCancel,
  onSave,
}: {
  member: Member | null
  roles: Role[]
  isSaving: boolean
  onCancel: () => void
  onSave: (member: Member, roles: string[]) => void
}) {
  return (
    <Dialog open={!!member} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent>
        {member && (
          <RolePicker
            // Keyed on the member, so opening it for somebody else starts from
            // what that person holds rather than from the last edit.
            key={member.id}
            member={member}
            roles={roles}
            isSaving={isSaving}
            onCancel={onCancel}
            onSave={onSave}
          />
        )}
      </DialogContent>
    </Dialog>
  )
}

function RolePicker({
  member,
  roles,
  isSaving,
  onCancel,
  onSave,
}: {
  member: Member
  roles: Role[]
  isSaving: boolean
  onCancel: () => void
  onSave: (member: Member, roles: string[]) => void
}) {
  const [granted, setGranted] = useState<string[]>(member.roles.map((role) => role.id))

  const toggle = (id: string) =>
    setGranted((current) =>
      current.includes(id) ? current.filter((held) => held !== id) : [...current, id],
    )

  return (
    <>
      <DialogHeader>
        <DialogTitle>What {member.name} may do</DialogTitle>
        <DialogDescription>
          Granting nothing leaves them in the organisation able to do nothing. That is not the same
          as removing them.
        </DialogDescription>
      </DialogHeader>

      {roles.length === 0 ? (
        <p className='text-sm text-muted-foreground'>
          This organisation has no roles yet. Create one before granting anything.
        </p>
      ) : (
        <div className='flex flex-wrap gap-2'>
          {roles.map((role) => (
            <button
              key={role.id}
              type='button'
              aria-pressed={granted.includes(role.id)}
              onClick={() => toggle(role.id)}
              className={cn(
                'rounded-md border px-2.5 py-1 text-sm transition-colors',
                granted.includes(role.id)
                  ? 'border-primary bg-primary/10 text-foreground'
                  : 'text-muted-foreground hover:bg-muted',
              )}
            >
              {role.name}
            </button>
          ))}
        </div>
      )}

      <DialogFooter>
        <Button variant='outline' onClick={onCancel}>
          Cancel
        </Button>
        <Button disabled={isSaving} onClick={() => onSave(member, granted)}>
          {isSaving && <Spinner className='size-3.5' />}
          Save
        </Button>
      </DialogFooter>
    </>
  )
}
