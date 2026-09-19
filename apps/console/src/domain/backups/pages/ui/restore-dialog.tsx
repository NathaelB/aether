import { useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { cn } from '@/lib/utils'
import { readableAge } from '../../archives'
import { checkRestoreName } from '../../restore'
import type { Backup } from '../../schedule'

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  archive: Backup | null
  isSubmitting: boolean
  refusal?: string
  onRestore: (name: string) => void
}

/**
 * Says what a restore is about to do before it does it.
 *
 * A new deployment and nothing else: the source is never in this dialog,
 * because a restore never touches it.
 */
export function RestoreDialog({
  open,
  onOpenChange,
  archive,
  isSubmitting,
  refusal,
  onRestore,
}: Props) {
  const [name, setName] = useState('')
  const [touched, setTouched] = useState(false)

  const problem = checkRestoreName(name)
  const canSubmit = !problem && !isSubmitting

  const close = (nextOpen: boolean) => {
    if (!nextOpen) {
      setName('')
      setTouched(false)
    }
    onOpenChange(nextOpen)
  }

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Restore this archive</DialogTitle>
          <DialogDescription>
            {archive
              ? `This provisions a new deployment, bootstrapped from the archive taken ${readableAge(
                  archive.finished_at,
                )}. It gets its own name and its own hostname. This deployment is not touched \
— nothing about it changes until you decide otherwise.`
              : null}
          </DialogDescription>
        </DialogHeader>

        <div className='space-y-2'>
          <Label htmlFor='restore-name'>Name for the recovery</Label>
          <Input
            id='restore-name'
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            onBlur={() => setTouched(true)}
            placeholder='acme-recovery'
            aria-invalid={touched && !!problem}
            className={cn(touched && problem && 'border-destructive')}
          />
          {touched && problem ? <p className='text-xs text-destructive'>{problem}</p> : null}
          {refusal ? <p className='text-xs text-destructive'>{refusal}</p> : null}
        </div>

        <DialogFooter>
          <Button variant='outline' onClick={() => close(false)}>
            Cancel
          </Button>
          <Button
            disabled={!canSubmit}
            onClick={() => {
              if (!canSubmit) {
                setTouched(true)
                return
              }
              onRestore(name.trim())
            }}
          >
            {isSubmitting ? <Spinner className='size-3.5' /> : null}
            {isSubmitting ? 'Restoring' : 'Restore'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
