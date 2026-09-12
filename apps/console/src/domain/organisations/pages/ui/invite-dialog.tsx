import { useState } from 'react'
import { Copy, Check } from 'lucide-react'
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
import { alreadyHere, checkEmail, type Invitation, type Member, type Role } from '../../members'

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  members: Member[]
  invitations: Invitation[]
  roles: Role[]
  isSaving: boolean
  onInvite: (email: string, roles: string[]) => void
  /** The link, once the platform has answered. Shown here and nowhere else. */
  link: string | null
}

export function InviteDialog({
  open,
  onOpenChange,
  members,
  invitations,
  roles,
  isSaving,
  onInvite,
  link,
}: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        {link ? (
          <TheLink link={link} onDone={() => onOpenChange(false)} />
        ) : (
          <InviteForm
            members={members}
            invitations={invitations}
            roles={roles}
            isSaving={isSaving}
            onInvite={onInvite}
            onCancel={() => onOpenChange(false)}
          />
        )}
      </DialogContent>
    </Dialog>
  )
}

/**
 * The one place the secret is ever shown.
 *
 * Deliberately a state somebody has to dismiss rather than a line in a list:
 * closing this is the last chance to have it, and the copy says so before
 * they find out.
 */
function TheLink({ link, onDone }: { link: string; onDone: () => void }) {
  const [copied, setCopied] = useState(false)

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(link)
      setCopied(true)
    } catch {
      // A browser that refuses the clipboard is not a failure worth a banner:
      // the link is on screen and can be selected.
      setCopied(false)
    }
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>Send them this link</DialogTitle>
        <DialogDescription>
          It is shown once. Closing this box is the last chance to copy it — nothing can show it
          again, and a new invitation means a new link.
        </DialogDescription>
      </DialogHeader>

      <div className='flex items-center gap-2'>
        <Input readOnly value={link} className='font-mono text-xs' aria-label='Invitation link' />
        <Button variant='outline' size='icon' aria-label='Copy the link' onClick={copy}>
          {copied ? <Check className='h-4 w-4' /> : <Copy className='h-4 w-4' />}
        </Button>
      </div>

      <DialogFooter>
        <Button onClick={onDone}>{copied ? 'Done' : 'Close without copying'}</Button>
      </DialogFooter>
    </>
  )
}

function InviteForm({
  members,
  invitations,
  roles,
  isSaving,
  onInvite,
  onCancel,
}: {
  members: Member[]
  invitations: Invitation[]
  roles: Role[]
  isSaving: boolean
  onInvite: (email: string, roles: string[]) => void
  onCancel: () => void
}) {
  const [email, setEmail] = useState('')
  const [touched, setTouched] = useState(false)
  const [granted, setGranted] = useState<string[]>([])

  const malformed = checkEmail(email)
  const duplicate = malformed ? null : alreadyHere(email, members, invitations, new Date())
  const problem = touched && email.trim().length > 0 ? (malformed ?? duplicate) : null
  const canInvite = !malformed && !duplicate && !isSaving

  const toggle = (id: string) =>
    setGranted((current) =>
      current.includes(id) ? current.filter((held) => held !== id) : [...current, id],
    )

  return (
    <>
      <DialogHeader>
        <DialogTitle>Invite somebody</DialogTitle>
        <DialogDescription>
          No mail is sent. You will be given a link to pass on, and whoever opens it has to be
          signed in as this address.
        </DialogDescription>
      </DialogHeader>

      <div className='space-y-4'>
        <div className='space-y-2'>
          <Label htmlFor='invite-email'>Email</Label>
          <Input
            id='invite-email'
            autoFocus
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            onBlur={() => setTouched(true)}
            placeholder='colleague@acme.com'
            aria-invalid={!!problem}
            className={cn(problem && 'border-destructive')}
          />
          {problem && <p className='text-xs text-destructive'>{problem}</p>}
        </div>

        <div className='space-y-2'>
          <Label>Roles</Label>
          {roles.length === 0 ? (
            <p className='text-xs text-muted-foreground'>
              This organisation has no roles yet. They can join without one and be granted
              something later.
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
                    'rounded-md border px-2.5 py-1 text-xs transition-colors',
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
        </div>
      </div>

      <DialogFooter>
        <Button variant='outline' onClick={onCancel}>
          Cancel
        </Button>
        <Button
          disabled={!canInvite}
          onClick={() => {
            if (!canInvite) {
              setTouched(true)
              return
            }
            onInvite(email.trim(), granted)
          }}
        >
          {isSaving && <Spinner className='size-3.5' />}
          {isSaving ? 'Inviting' : 'Invite'}
        </Button>
      </DialogFooter>
    </>
  )
}
