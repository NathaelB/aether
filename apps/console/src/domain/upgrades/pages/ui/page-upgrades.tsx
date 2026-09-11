import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import { EmptyState, Page, PageTitle, Section } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { StatusBadge, type Tone } from '@/components/ui/status-badge'
import { RISK_LABELS, RISK_TONES } from '@/domain/releases/status'
import { formatDistanceToNow } from 'date-fns'
import { CheckCircle2, Info } from 'lucide-react'
import {
  AUTO_UPGRADE_DESCRIPTIONS,
  AUTO_UPGRADE_LABELS,
  MAJOR_ALWAYS_ASKS,
  WEEKDAYS,
  offeredTimezones,
  policyIsInert,
} from '../../policy'
import type { UpgradeProgress } from '../../progress'
import { changeBetween, heldBack, nextUpgrade, whyHeldBack, type VersionChange } from '../../version'

interface Props {
  deployment?: Schemas.Deployment
  releases: Schemas.ReleaseAvailability[]
  progress: UpgradeProgress | null
  isLoading: boolean
  onUpgrade: (version: string) => void
  isUpgrading: boolean
  onSaveSettings: (settings: Schemas.SetUpgradeSettingsRequest) => void
  isSaving: boolean
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

const DURATIONS = [
  { value: 60, label: '1 hour' },
  { value: 120, label: '2 hours' },
  { value: 240, label: '4 hours' },
  { value: 480, label: '8 hours' },
]

function Notice({ children }: { children: React.ReactNode }) {
  return (
    <div className='flex items-start gap-2 rounded-md border bg-muted/40 px-3 py-2 text-sm text-muted-foreground'>
      <Info className='mt-0.5 h-4 w-4 shrink-0' />
      <span>{children}</span>
    </div>
  )
}

export function PageUpgrades({
  deployment,
  releases,
  progress,
  isLoading,
  onUpgrade,
  isUpgrading,
  onSaveSettings,
  isSaving,
}: Props) {
  if (isLoading || !deployment) {
    return (
      <Page>
        <Skeleton className='h-9 w-64' />
        <Skeleton className='mt-8 h-40 w-full' />
      </Page>
    )
  }

  const available = nextUpgrade(deployment.version, releases)
  const change = available ? changeBetween(deployment.version, available.id.version) : null
  const waiting = heldBack(deployment.version, releases)

  return (
    <Page>
      <PageTitle
        title='Upgrades'
        badges={
          <span className='text-xs text-muted-foreground'>
            {deployment.name} · running {deployment.version}
          </span>
        }
      />

      <div className='mt-8 space-y-8'>
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
                  {change === 'major' ? 'Approve and upgrade' : 'Upgrade now'}
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

        <UpgradeSettings
          // Remounted when the saved settings change, so a refetch wins over
          // an open form. Whoever was editing at that moment was editing
          // something the platform no longer holds.
          key={settingsKey(deployment)}
          deployment={deployment}
          onSave={onSaveSettings}
          isSaving={isSaving}
        />
      </div>
    </Page>
  )
}

function InProgress({ progress }: { progress: UpgradeProgress }) {
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
            Step {progress.step} of {progress.of}
          </StatusBadge>
        </div>
        <div className='flex gap-1'>
          {Array.from({ length: progress.of }, (_, index) => (
            <span
              key={index}
              className={
                index < progress.step
                  ? 'h-1.5 flex-1 rounded-full bg-primary'
                  : 'h-1.5 flex-1 rounded-full bg-muted'
              }
            />
          ))}
        </div>
        <p className='text-xs text-muted-foreground'>
          Started {formatDistanceToNow(new Date(progress.startedAt))} ago.
          {progress.of > 1 && ' Versions in between are applied one after the other.'}
        </p>
      </div>
    </Section>
  )
}

function UpgradeSettings({
  deployment,
  onSave,
  isSaving,
}: {
  deployment: Schemas.Deployment
  onSave: (settings: Schemas.SetUpgradeSettingsRequest) => void
  isSaving: boolean
}) {
  const [policy, setPolicy] = useState<Schemas.AutoUpgradePolicy>(deployment.auto_upgrade)
  const [day, setDay] = useState(deployment.maintenance_window?.day ?? 'sun')
  const [start, setStart] = useState(deployment.maintenance_window?.start ?? '03:00')
  const [minutes, setMinutes] = useState(deployment.maintenance_window?.duration ?? 120)
  const [timezone, setTimezone] = useState(
    deployment.maintenance_window?.timezone ?? offeredTimezones()[0],
  )
  const [hasWindow, setHasWindow] = useState(!!deployment.maintenance_window)

  const inert = policyIsInert(policy, hasWindow)

  return (
    <Section title='Automatic upgrades'>
      <div className='space-y-5 rounded-lg border p-5'>
        <div className='space-y-2'>
          <Label htmlFor='policy'>What we may apply for you</Label>
          <Select
            value={policy}
            onValueChange={(value) => setPolicy(value as Schemas.AutoUpgradePolicy)}
          >
            <SelectTrigger id='policy' className='w-full sm:w-96'>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {(Object.keys(AUTO_UPGRADE_LABELS) as Schemas.AutoUpgradePolicy[]).map((value) => (
                <SelectItem key={value} value={value}>
                  {AUTO_UPGRADE_LABELS[value]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <p className='text-sm text-muted-foreground'>{AUTO_UPGRADE_DESCRIPTIONS[policy]}</p>
        </div>

        <div className='space-y-3'>
          <div className='flex items-center justify-between gap-4'>
            <Label htmlFor='window'>Maintenance window</Label>
            <Button variant='ghost' size='sm' onClick={() => setHasWindow(!hasWindow)}>
              {hasWindow ? 'Remove' : 'Add a window'}
            </Button>
          </div>

          {hasWindow ? (
            <div className='grid gap-3 sm:grid-cols-4'>
              <Select value={day} onValueChange={setDay}>
                <SelectTrigger id='window'>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {WEEKDAYS.map(({ value, label }) => (
                    <SelectItem key={value} value={value}>
                      {label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Input type='time' value={start} onChange={(event) => setStart(event.target.value)} />
              <Select value={String(minutes)} onValueChange={(value) => setMinutes(Number(value))}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {DURATIONS.map(({ value, label }) => (
                    <SelectItem key={value} value={String(value)}>
                      {label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Select value={timezone} onValueChange={setTimezone}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {offeredTimezones().map((zone) => (
                    <SelectItem key={zone} value={zone}>
                      {zone}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          ) : (
            <p className='text-sm text-muted-foreground'>
              Without a window nothing is applied for you, whatever is chosen above.
            </p>
          )}
        </div>

        {inert && (
          <Notice>
            This does nothing until there is a window to work in. Add one, or leave the policy on
            {` ${AUTO_UPGRADE_LABELS.manual.toLowerCase()}`}.
          </Notice>
        )}
        <Notice>{MAJOR_ALWAYS_ASKS}</Notice>

        <div className='flex justify-end'>
          <Button
            size='sm'
            disabled={isSaving}
            onClick={() =>
              onSave({
                auto_upgrade: policy,
                maintenance_window: hasWindow ? { day, start, minutes, timezone } : null,
              })
            }
          >
            Save
          </Button>
        </div>
      </div>
    </Section>
  )
}

function settingsKey(deployment: Schemas.Deployment): string {
  const window = deployment.maintenance_window

  return [
    deployment.auto_upgrade,
    window?.day ?? '',
    window?.start ?? '',
    window?.duration ?? '',
    window?.timezone ?? '',
  ].join('|')
}
