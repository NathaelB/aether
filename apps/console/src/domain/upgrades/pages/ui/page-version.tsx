import type { Schemas } from '@/api/api.client'
import { EmptyState, Section, SettingsPage } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { useTickingClock } from '@/hooks/use-ticking-clock'
import { StatusBadge, type Tone } from '@/components/ui/status-badge'
import { RISK_LABELS, RISK_TONES } from '@/domain/releases/status'
import { formatDistanceToNow } from 'date-fns'
import { CheckCircle2 } from 'lucide-react'
import { cn } from '@/lib/utils'
import { MAJOR_ALWAYS_ASKS } from '../../policy'
import type { UpgradeProgress } from '../../progress'
import { changeBetween, heldBack, nextUpgrade, whyHeldBack, type VersionChange } from '../../version'
import { Notice } from './notice'

interface Props {
  deployment?: Schemas.Deployment
  releases: Schemas.ReleaseAvailability[]
  progress: UpgradeProgress | null
  isLoading: boolean
  onUpgrade: (version: string) => void
  isUpgrading: boolean
}

const CHANGE_LABELS: Record<VersionChange, string> = {
  patch: 'Patch',
  minor: 'Minor',
  major: 'Major',
}

const CHANGE_TONES: Record<VersionChange, Tone> = {
  patch: 'neutral',
  minor: 'accent',
  major: 'warning',
}

export function PageVersion({
  deployment,
  releases,
  progress,
  isLoading,
  onUpgrade,
  isUpgrading,
}: Props) {
  if (isLoading || !deployment) {
    return <Skeleton className='h-64 w-full' />
  }

  const available = nextUpgrade(deployment.version, releases)
  const change = available ? changeBetween(deployment.version, available.id.version) : null
  const waiting = heldBack(deployment.version, releases)

  return (
    <SettingsPage
      title='Version'
      description={`Running ${deployment.version}.`}
    >
      <div className='space-y-8'>
        {progress && <InProgress progress={progress} />}

        <Section title='Available upgrade'>
          {!available || !change ? (
            <EmptyState
              icon={<CheckCircle2 className='h-5 w-5' />}
              title={`You are on ${deployment.version}`}
              description={
                waiting
                  ? `${waiting.id.version} exists. ${whyHeldBack(waiting.reason)}`
                  : 'There is nothing newer to move to right now.'
              }
            />
          ) : (
            <div className='space-y-4 rounded-lg border p-5'>
              <div className='flex flex-wrap items-center justify-between gap-3'>
                <div className='flex flex-wrap items-center gap-2'>
                  <span className='font-mono text-lg font-semibold'>{available.id.version}</span>
                  <StatusBadge tone={CHANGE_TONES[change]} dot={false}>
                    {CHANGE_LABELS[change]}
                  </StatusBadge>
                  <StatusBadge tone={RISK_TONES[available.risk]} dot={false}>
                    {RISK_LABELS[available.risk]}
                  </StatusBadge>
                </div>
                <Button
                  size='sm'
                  disabled={isUpgrading || !!progress}
                  onClick={() => onUpgrade(available.id.version)}
                >
                  {isUpgrading && <Spinner className='size-3.5' />}
                  {isUpgrading
                    ? 'Starting'
                    : change === 'major'
                      ? 'Approve and upgrade'
                      : 'Upgrade now'}
                </Button>
              </div>

              {change === 'major' && <Notice>{MAJOR_ALWAYS_ASKS}</Notice>}

              {waiting && (
                <Notice>
                  {waiting.id.version} exists too. {whyHeldBack(waiting.reason)}
                </Notice>
              )}

              <div>
                <p className='mb-2 text-xs font-medium text-muted-foreground'>What changed</p>
                <div className='max-h-72 overflow-y-auto whitespace-pre-wrap rounded-md border bg-muted/30 p-3 text-sm'>
                  {available.notes.trim() || 'No notes were published with this version.'}
                </div>
              </div>
            </div>
          )}
        </Section>
      </div>
    </SettingsPage>
  )
}

function InProgress({ progress }: { progress: UpgradeProgress }) {
  // Re-renders so the elapsed time keeps moving. A screen that does not move
  // during a long upgrade reads as a screen that has stopped working.
  useTickingClock(10_000)

  const step = Math.min(progress.step, progress.of)

  return (
    <Section title='Upgrade in progress'>
      <div className='space-y-3 rounded-lg border p-5'>
        <div className='flex flex-wrap items-center justify-between gap-3'>
          <div className='flex items-center gap-2 font-mono text-sm'>
            <span className='text-muted-foreground'>{progress.from}</span>
            <span className='text-muted-foreground'>&rarr;</span>
            <span className='font-semibold'>{progress.to}</span>
          </div>
          <StatusBadge tone='progress'>
            Step {step} of {progress.of}
          </StatusBadge>
        </div>

        <div
          className='flex gap-1'
          role='progressbar'
          aria-valuemin={0}
          aria-valuemax={progress.of}
          // The step being worked on is under way, not finished, so what is
          // claimed as done is the ones behind it.
          aria-valuenow={step - 1}
          aria-valuetext={`Step ${step} of ${progress.of}, applying ${progress.to}`}
        >
          {Array.from({ length: progress.of }, (_, index) => {
            const done = index < step - 1
            const current = index === step - 1

            return (
              <span
                key={index}
                className={cn(
                  'h-1.5 flex-1 rounded-full',
                  done && 'bg-primary',
                  current && 'upgrade-step-active',
                  !done && !current && 'bg-muted',
                )}
              />
            )
          })}
        </div>

        <p className='text-xs text-muted-foreground'>
          Started {formatDistanceToNow(new Date(progress.startedAt))} ago.
          {progress.of > 1 && ' Versions in between are applied one after the other.'}
        </p>
      </div>
    </Section>
  )
}
