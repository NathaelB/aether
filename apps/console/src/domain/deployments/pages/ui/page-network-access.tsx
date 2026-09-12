import { useState } from 'react'
import { Globe, Plus, Trash2 } from 'lucide-react'
import type { Schemas } from '@/api/api.client'
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
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { StatusBadge } from '@/components/ui/status-badge'
import { cn } from '@/lib/utils'
import {
  checkAddition,
  describeAccess,
  describeAdditionProblem,
  describeRemoval,
  rangesOf,
  withRange,
  withoutRange,
  type NetworkAccess,
} from '../../network-access'

interface Props {
  deployment?: Schemas.Deployment
  access?: NetworkAccess
  isLoading: boolean
  onApply: (allowed: string[]) => void
  isSaving: boolean
}

export function PageNetworkAccess({ deployment, access, isLoading, onApply, isSaving }: Props) {
  const [adding, setAdding] = useState(false)

  if (isLoading || !deployment) {
    return <Skeleton className='h-64 w-full' />
  }

  const applied = rangesOf(access)
  const hostname = deployment.namespace

  return (
    <SettingsPage
      title='Network access'
      description='Which source addresses may reach this deployment.'
    >
      <Section
        title='Applied now'
        aside={
          <Button size='sm' onClick={() => setAdding(true)} disabled={isSaving}>
            <Plus className='h-3.5 w-3.5' />
            Add a range
          </Button>
        }
      >
        {applied.length === 0 ? (
          <EmptyState
            icon={<Globe className='h-5 w-5' />}
            title='Reachable from anywhere'
            description='No restriction is in place. Any address can reach this deployment.'
          />
        ) : (
          <div className='overflow-hidden rounded-lg border'>
            <div className='flex items-center gap-2 border-b bg-muted/30 px-4 py-2.5'>
              <StatusBadge tone='warning' dot={false}>
                Restricted
              </StatusBadge>
              <span className='text-xs text-muted-foreground'>
                {describeAccess(applied, hostname)}
              </span>
            </div>
            <ul>
              {applied.map((range) => (
                <li
                  key={range}
                  className='flex items-center justify-between gap-3 border-b px-4 py-3 last:border-b-0'
                >
                  <span className='font-mono text-sm'>{range}</span>
                  <Button
                    variant='outline'
                    size='icon'
                    aria-label={`Remove ${range}`}
                    title={describeRemoval(applied, hostname) ?? `Remove ${range}`}
                    disabled={isSaving}
                    onClick={() => onApply(withoutRange(applied, range))}
                  >
                    <Trash2 className='h-4 w-4' />
                  </Button>
                </li>
              ))}
            </ul>
            {describeRemoval(applied, hostname) && (
              <p className='border-t bg-muted/20 px-4 py-2.5 text-xs text-muted-foreground'>
                {describeRemoval(applied, hostname)}
              </p>
            )}
          </div>
        )}
      </Section>

      <AddRangeDialog
        open={adding}
        onOpenChange={setAdding}
        applied={applied}
        hostname={hostname}
        isSaving={isSaving}
        onAdd={(range) => onApply(withRange(applied, range))}
      />
    </SettingsPage>
  )
}

function AddRangeDialog({
  open,
  onOpenChange,
  applied,
  hostname,
  isSaving,
  onAdd,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  applied: string[]
  hostname: string
  isSaving: boolean
  onAdd: (range: string) => void
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        {/*
          The box holds its own state and lives only while the dialog is open,
          so it starts empty every time it is reached. Resetting it from an
          effect would mean writing state during a render the close already
          settled.
        */}
        <AddRangeForm
          applied={applied}
          hostname={hostname}
          isSaving={isSaving}
          onAdd={(range) => {
            onAdd(range)
            onOpenChange(false)
          }}
          onCancel={() => onOpenChange(false)}
        />
      </DialogContent>
    </Dialog>
  )
}

function AddRangeForm({
  applied,
  hostname,
  isSaving,
  onAdd,
  onCancel,
}: {
  applied: string[]
  hostname: string
  isSaving: boolean
  onAdd: (range: string) => void
  onCancel: () => void
}) {
  const [value, setValue] = useState('')
  const [touched, setTouched] = useState(false)

  const problem = checkAddition(applied, value)
  // Not while the box is still untouched: telling somebody an empty field is
  // not a range, before they have typed anything, is noise.
  const shown = touched && value.trim().length > 0 ? problem : null
  const canAdd = !problem && !isSaving

  const submit = () => {
    if (!canAdd) {
      setTouched(true)
      return
    }
    onAdd(value)
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>Add an allowed range</DialogTitle>
        <DialogDescription>
          {applied.length === 0
            ? `${hostname} is reachable from anywhere today. Adding a range restricts it to that range.`
            : `Addresses in this range will reach ${hostname}, on top of the ones already allowed.`}
        </DialogDescription>
      </DialogHeader>

      <div className='space-y-2'>
        <Label htmlFor='allowed-range'>Range</Label>
        <Input
          id='allowed-range'
          autoFocus
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onBlur={() => setTouched(true)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault()
              submit()
            }
          }}
          placeholder='203.0.113.0/24'
          aria-invalid={!!shown}
          aria-describedby={shown ? 'allowed-range-problem' : 'allowed-range-hint'}
          className={cn('font-mono', shown && 'border-destructive')}
        />
        {shown ? (
          <p id='allowed-range-problem' className='text-xs text-destructive'>
            {describeAdditionProblem(shown, value.trim())}
          </p>
        ) : (
          <p id='allowed-range-hint' className='text-xs text-muted-foreground'>
            A single address is a /32, so 203.0.113.9 is written 203.0.113.9/32.
          </p>
        )}
      </div>

      <DialogFooter>
        <Button variant='outline' onClick={onCancel}>
          Cancel
        </Button>
        <Button onClick={submit} disabled={!canAdd}>
          {isSaving && <Spinner className='size-3.5' />}
          {isSaving ? 'Adding' : 'Add'}
        </Button>
      </DialogFooter>
    </>
  )
}
