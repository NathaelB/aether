import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import { Section } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Spinner } from '@/components/ui/spinner'
import { describeCutover, otherDeployments } from '../../cutover'

interface Props {
  self: Schemas.Deployment
  deployments: Schemas.Deployment[]
  isSubmitting: boolean
  refusal?: string
  onCutover: (other: Schemas.Deployment) => void
}

/**
 * The second step after a restore: naming what moves, and what happens to
 * the deployment it moves away from, before it happens.
 *
 * Symmetric, the same way the platform's own cutover is: offered here
 * whether this deployment is a fresh recovery about to take over, or the one
 * currently live about to cut back to it. Which direction it is does not
 * change what the dialog has to say.
 */
export function CutoverSection({ self, deployments, isSubmitting, refusal, onCutover }: Props) {
  const [selectedId, setSelectedId] = useState('')
  const [confirming, setConfirming] = useState(false)

  const candidates = otherDeployments(deployments, self.id)
  const selected = candidates.find((candidate) => candidate.id === selectedId) ?? null

  return (
    <Section title='Cutover'>
      <div className='space-y-3 rounded-lg border p-4'>
        <p className='text-xs text-muted-foreground'>
          Move the hostname this deployment answers at, to or from another deployment in this
          organisation. This is the second step after a restore, once the recovery has been
          inspected and trusted -- and it can be reversed the same way, by cutting over again in
          the other direction.
        </p>

        {candidates.length === 0 ? (
          <p className='text-xs text-muted-foreground'>
            No other deployment in this organisation to cut over with.
          </p>
        ) : (
          <div className='flex flex-wrap items-center gap-2'>
            <Select value={selectedId} onValueChange={setSelectedId}>
              <SelectTrigger className='w-64'>
                <SelectValue placeholder='Choose a deployment' />
              </SelectTrigger>
              <SelectContent>
                {candidates.map((candidate) => (
                  <SelectItem key={candidate.id} value={candidate.id}>
                    {candidate.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button size='sm' disabled={!selected} onClick={() => setConfirming(true)}>
              Cut over
            </Button>
          </div>
        )}
      </div>

      <Dialog
        open={confirming && !!selected}
        onOpenChange={(open) => {
          if (!open) setConfirming(false)
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Cut over "{self.name}"</DialogTitle>
            <DialogDescription>{selected ? describeCutover(self, selected) : null}</DialogDescription>
          </DialogHeader>

          {refusal ? <p className='text-xs text-destructive'>{refusal}</p> : null}

          <DialogFooter>
            <Button variant='outline' onClick={() => setConfirming(false)}>
              Cancel
            </Button>
            <Button
              disabled={isSubmitting || !selected}
              onClick={() => {
                if (selected) onCutover(selected)
              }}
            >
              {isSubmitting ? <Spinner className='size-3.5' /> : null}
              {isSubmitting ? 'Cutting over' : 'Confirm cutover'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Section>
  )
}
