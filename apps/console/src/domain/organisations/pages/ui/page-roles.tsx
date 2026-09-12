import { useState } from 'react'
import { Pencil, Plus, ShieldCheck, Trash2 } from 'lucide-react'
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
import {
  describeDeletion,
  heldByCount,
  summarise,
  type Member,
  type Role,
} from '../../permissions'
import { RoleDialog } from './role-dialog'

interface Props {
  roles: Role[]
  members: Member[]
  isLoading: boolean
  isSaving: boolean
  onCreate: (name: string, permissions: number) => void
  onUpdate: (role: Role, name: string, permissions: number) => void
  onDelete: (role: Role) => void
}

export function PageRoles({
  roles,
  members,
  isLoading,
  isSaving,
  onCreate,
  onUpdate,
  onDelete,
}: Props) {
  const [editing, setEditing] = useState<Role | null>(null)
  const [creating, setCreating] = useState(false)
  const [deleting, setDeleting] = useState<Role | null>(null)

  if (isLoading) {
    return <Skeleton className='h-64 w-full' />
  }

  return (
    <SettingsPage
      title='Roles'
      description='What this organisation can grant. The owner holds everything without a role.'
    >
      <Section
        title='Roles'
        aside={
          <Button size='sm' onClick={() => setCreating(true)} disabled={isSaving}>
            <Plus className='h-3.5 w-3.5' />
            New role
          </Button>
        }
      >
        {roles.length === 0 ? (
          <EmptyState
            icon={<ShieldCheck className='h-5 w-5' />}
            title='No roles yet'
            description='Until there is one, only the owner can do anything here. Members can be invited and will wait for something to hold.'
          />
        ) : (
          <div className='overflow-hidden rounded-lg border'>
            <ul>
              {roles.map((role) => {
                const held = heldByCount(role, members)

                return (
                  <li
                    key={role.id}
                    className='flex items-start justify-between gap-3 border-b px-4 py-3 last:border-b-0'
                  >
                    <div className='min-w-0'>
                      <div className='flex items-center gap-2'>
                        <span className='text-sm font-medium'>{role.name}</span>
                        <StatusBadge tone={held > 0 ? 'accent' : 'neutral'} dot={false}>
                          {held === 0 ? 'Held by nobody' : held === 1 ? '1 member' : `${held} members`}
                        </StatusBadge>
                      </div>
                      <p className='mt-0.5 text-xs text-muted-foreground'>{summarise(role)}</p>
                    </div>

                    <div className='flex shrink-0 items-center gap-2'>
                      <Button
                        variant='outline'
                        size='icon'
                        aria-label={`Edit ${role.name}`}
                        disabled={isSaving}
                        onClick={() => setEditing(role)}
                      >
                        <Pencil className='h-4 w-4' />
                      </Button>
                      <Button
                        variant='outline'
                        size='icon'
                        aria-label={`Delete ${role.name}`}
                        disabled={isSaving}
                        onClick={() => setDeleting(role)}
                      >
                        <Trash2 className='h-4 w-4' />
                      </Button>
                    </div>
                  </li>
                )
              })}
            </ul>
          </div>
        )}
      </Section>

      <RoleDialog
        open={creating || !!editing}
        onOpenChange={(open) => {
          if (!open) {
            setCreating(false)
            setEditing(null)
          }
        }}
        editing={editing}
        existing={roles}
        isSaving={isSaving}
        onSave={(name, permissions) => {
          if (editing) onUpdate(editing, name, permissions)
          else onCreate(name, permissions)
          setCreating(false)
          setEditing(null)
        }}
      />

      <ConfirmDeletion
        role={deleting}
        members={members}
        isSaving={isSaving}
        onCancel={() => setDeleting(null)}
        onConfirm={(role) => {
          onDelete(role)
          setDeleting(null)
        }}
      />
    </SettingsPage>
  )
}

/**
 * Says how many people lose what, before it happens.
 *
 * Deleting a role held by members takes their rights away immediately, with
 * no new sign-in. A count is the difference between a decision and a
 * surprise.
 */
function ConfirmDeletion({
  role,
  members,
  isSaving,
  onCancel,
  onConfirm,
}: {
  role: Role | null
  members: Member[]
  isSaving: boolean
  onCancel: () => void
  onConfirm: (role: Role) => void
}) {
  return (
    <Dialog open={!!role} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent>
        {role && (
          <>
            <DialogHeader>
              <DialogTitle>Delete {role.name}?</DialogTitle>
              <DialogDescription>{describeDeletion(role, members)}</DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button variant='outline' onClick={onCancel}>
                Cancel
              </Button>
              <Button disabled={isSaving} onClick={() => onConfirm(role)}>
                {isSaving && <Spinner className='size-3.5' />}
                Delete
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}
