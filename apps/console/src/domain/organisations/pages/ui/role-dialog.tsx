import { useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { cn } from '@/lib/utils'
import {
  GROUPS,
  checkName,
  heldBy,
  maskFrom,
  permissionsIn,
  unmodelled,
  type Role,
} from '../../permissions'

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Absent means a new one. */
  editing: Role | null
  existing: Role[]
  isSaving: boolean
  onSave: (name: string, permissions: number) => void
}

export function RoleDialog({ open, onOpenChange, editing, existing, isSaving, onSave }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='max-h-[85vh] overflow-y-auto sm:max-w-2xl'>
        {open && (
          <RoleForm
            // Keyed on what is being edited, so opening it for another role
            // starts from that role rather than from the last edit.
            key={editing?.id ?? 'new'}
            editing={editing}
            existing={existing}
            isSaving={isSaving}
            onSave={onSave}
            onCancel={() => onOpenChange(false)}
          />
        )}
      </DialogContent>
    </Dialog>
  )
}

function RoleForm({
  editing,
  existing,
  isSaving,
  onSave,
  onCancel,
}: {
  editing: Role | null
  existing: Role[]
  isSaving: boolean
  onSave: (name: string, permissions: number) => void
  onCancel: () => void
}) {
  const [name, setName] = useState(editing?.name ?? '')
  const [touched, setTouched] = useState(false)
  const [chosen, setChosen] = useState<number[]>(heldBy(editing?.permissions ?? 0))

  // Anything the catalogue cannot show is carried through untouched. Dropping
  // it would take rights away on a save that looks like one checkbox.
  const carriedOver = unmodelled(editing?.permissions ?? 0)

  const problem = checkName(name, existing, editing ?? undefined)
  const shown = touched ? problem : null
  const canSave = !problem && !isSaving

  const toggle = (bit: number) =>
    setChosen((current) =>
      current.includes(bit) ? current.filter((held) => held !== bit) : [...current, bit]
    )

  return (
    <>
      <DialogHeader>
        <DialogTitle>{editing ? `Edit ${editing.name}` : 'New role'}</DialogTitle>
        <DialogDescription>
          A role is a set of permissions somebody can be granted. Changes take effect on the next
          request, for everybody holding it — no new sign-in.
        </DialogDescription>
      </DialogHeader>

      <div className='space-y-5'>
        <div className='space-y-2'>
          <Label htmlFor='role-name'>Name</Label>
          <Input
            id='role-name'
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            onBlur={() => setTouched(true)}
            placeholder='operator'
            aria-invalid={!!shown}
            className={cn(shown && 'border-destructive')}
          />
          {shown && <p className='text-xs text-destructive'>{shown}</p>}
        </div>

        {carriedOver > 0 && (
          <p className='rounded-md border bg-muted/30 p-3 text-xs text-muted-foreground'>
            This role also carries permissions this console does not know about. They are kept as
            they are.
          </p>
        )}

        <div className='space-y-4'>
          {GROUPS.map((group) => (
            <div key={group} className='space-y-2'>
              <p className='text-xs font-medium text-muted-foreground'>{group}</p>
              <div className='space-y-1'>
                {permissionsIn(group).map((permission) => {
                  const held = chosen.includes(permission.bit)

                  return (
                    <button
                      key={permission.bit}
                      type='button'
                      role='checkbox'
                      aria-checked={held}
                      aria-label={permission.label}
                      onClick={() => toggle(permission.bit)}
                      className={cn(
                        'flex w-full items-start gap-3 rounded-md border p-3 text-left transition-colors',
                        held ? 'border-primary bg-primary/5' : 'hover:bg-muted/50'
                      )}
                    >
                      <span
                        aria-hidden
                        className={cn(
                          'mt-0.5 h-4 w-4 shrink-0 rounded border',
                          held && 'border-primary bg-primary'
                        )}
                      />
                      <span className='min-w-0'>
                        <span className='block text-sm'>{permission.label}</span>
                        <span className='block text-xs text-muted-foreground'>
                          {permission.description}
                        </span>
                      </span>
                    </button>
                  )
                })}
              </div>
            </div>
          ))}
        </div>
      </div>

      <DialogFooter>
        <Button variant='outline' onClick={onCancel}>
          Cancel
        </Button>
        <Button
          disabled={!canSave}
          onClick={() => {
            if (!canSave) {
              setTouched(true)
              return
            }
            onSave(name.trim(), maskFrom(chosen, carriedOver))
          }}
        >
          {isSaving && <Spinner className='size-3.5' />}
          {editing ? 'Save' : 'Create'}
        </Button>
      </DialogFooter>
    </>
  )
}
