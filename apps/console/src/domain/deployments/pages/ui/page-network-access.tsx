import { useState } from 'react'
import { Globe, Lock, Plus, X } from 'lucide-react'
import type { Schemas } from '@/api/api.client'
import { EmptyState, Section, SettingsPage } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { StatusBadge } from '@/components/ui/status-badge'
import { cn } from '@/lib/utils'
import { Notice } from '@/domain/upgrades/pages/ui/notice'
import {
  accessFrom,
  describeAccess,
  describeProblem,
  duplicates,
  hasChanges,
  isOpen,
  problems,
  rangesOf,
  type NetworkAccess,
} from '../../network-access'

interface Props {
  deployment?: Schemas.Deployment
  access?: NetworkAccess
  isLoading: boolean
  onSave: (allowed: string[]) => void
  isSaving: boolean
}

export function PageNetworkAccess({ deployment, access, isLoading, onSave, isSaving }: Props) {
  if (isLoading || !deployment) {
    return <Skeleton className='h-64 w-full' />
  }

  return (
    <SettingsPage
      title='Network access'
      description='Which source addresses may reach this deployment.'
    >
      <AllowListForm
        // Remounted when what is applied changes, so a refetch wins over an
        // open form: whoever was editing was editing something the platform
        // no longer holds.
        key={rangesOf(access).join(',')}
        hostname={deployment.namespace}
        access={access}
        onSave={onSave}
        isSaving={isSaving}
      />
    </SettingsPage>
  )
}

function AllowListForm({
  hostname,
  access,
  onSave,
  isSaving,
}: {
  hostname: string
  access?: NetworkAccess
  onSave: (allowed: string[]) => void
  isSaving: boolean
}) {
  const applied = rangesOf(access)
  const [entries, setEntries] = useState<string[]>(applied.length > 0 ? applied : [''])

  const wrong = problems(entries)
  const repeated = duplicates(entries)
  const changed = hasChanges(entries, access)
  const next = accessFrom(entries)
  const canSave = wrong.size === 0 && changed && !isSaving

  const update = (index: number, value: string) =>
    setEntries((current) => current.map((entry, at) => (at === index ? value : entry)))

  const remove = (index: number) =>
    setEntries((current) => {
      const left = current.filter((_, at) => at !== index)
      // A form with no rows has nothing to type into. The empty row is what
      // "open" looks like while editing.
      return left.length > 0 ? left : ['']
    })

  return (
    <div className='space-y-8'>
      <Section title='Applied now'>
        {isOpen(access) ? (
          <EmptyState
            icon={<Globe className='h-5 w-5' />}
            title='Reachable from anywhere'
            description='No restriction is in place. Any address can reach this deployment.'
          />
        ) : (
          <div className='space-y-3 rounded-lg border p-5'>
            <div className='flex items-center gap-2'>
              <Lock className='h-4 w-4 text-muted-foreground' />
              <StatusBadge tone='warning' dot={false}>
                Restricted
              </StatusBadge>
              <span className='text-sm text-muted-foreground'>
                {applied.length === 1 ? '1 range' : `${applied.length} ranges`}
              </span>
            </div>
            <ul className='flex flex-wrap gap-2'>
              {applied.map((range) => (
                <li
                  key={range}
                  className='rounded-md border bg-muted/30 px-2 py-1 font-mono text-xs'
                >
                  {range}
                </li>
              ))}
            </ul>
          </div>
        )}
      </Section>

      <Section title='Allowed ranges'>
        <div className='space-y-4 rounded-lg border p-5'>
          <div className='space-y-2'>
            {entries.map((entry, index) => {
              const problem = wrong.get(index)
              const duplicate = repeated.has(index)

              return (
                <div key={index} className='space-y-1'>
                  <div className='flex items-center gap-2'>
                    <Input
                      value={entry}
                      onChange={(event) => update(index, event.target.value)}
                      placeholder='203.0.113.0/24'
                      aria-label={`Allowed range ${index + 1}`}
                      aria-invalid={!!problem}
                      className={cn('font-mono', problem && 'border-destructive')}
                    />
                    <Button
                      type='button'
                      variant='ghost'
                      size='icon'
                      aria-label={`Remove range ${index + 1}`}
                      onClick={() => remove(index)}
                    >
                      <X className='h-4 w-4' />
                    </Button>
                  </div>
                  {problem && (
                    <p className='text-xs text-destructive'>
                      {describeProblem(problem, entry.trim())}
                    </p>
                  )}
                  {!problem && duplicate && (
                    <p className='text-xs text-muted-foreground'>
                      Already in the list above. It will be kept once.
                    </p>
                  )}
                </div>
              )
            })}
          </div>

          <Button
            type='button'
            variant='outline'
            size='sm'
            onClick={() => setEntries((current) => [...current, ''])}
          >
            <Plus className='h-3.5 w-3.5' />
            Add a range
          </Button>

          <Notice>{describeAccess(entries, hostname)}</Notice>

          {next.kind === 'open' && applied.length > 0 && (
            <Notice>
              Saving with no ranges removes the restriction. It does not make the deployment
              unreachable.
            </Notice>
          )}

          <div className='flex justify-end'>
            <Button
              size='sm'
              disabled={!canSave}
              onClick={() => onSave(next.kind === 'open' ? [] : next.allowed)}
            >
              {isSaving && <Spinner className='size-3.5' />}
              {isSaving ? 'Saving' : 'Save'}
            </Button>
          </div>
        </div>
      </Section>
    </div>
  )
}
