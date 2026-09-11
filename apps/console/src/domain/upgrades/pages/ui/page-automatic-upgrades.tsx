import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import { SettingsPage } from '@/components/layout/page'
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
import {
  AUTO_UPGRADE_DESCRIPTIONS,
  AUTO_UPGRADE_LABELS,
  MAJOR_ALWAYS_ASKS,
  WEEKDAYS,
  offeredTimezones,
  policyIsInert,
} from '../../policy'
import { Notice } from './notice'

const DURATIONS = [
  { value: 60, label: '1 hour' },
  { value: 120, label: '2 hours' },
  { value: 240, label: '4 hours' },
  { value: 480, label: '8 hours' },
]

interface Props {
  deployment?: Schemas.Deployment
  isLoading: boolean
  onSave: (settings: Schemas.SetUpgradeSettingsRequest) => void
  isSaving: boolean
}

export function PageAutomaticUpgrades({ deployment, isLoading, onSave, isSaving }: Props) {
  if (isLoading || !deployment) {
    return <Skeleton className='h-64 w-full' />
  }

  return (
    <SettingsPage
      title='Automatic upgrades'
      description='What the platform may apply without asking, and when.'
    >
      <UpgradeSettings
        // Remounted when the saved settings change, so a refetch wins over an
        // open form. Whoever was editing at that moment was editing something
        // the platform no longer holds.
        key={settingsKey(deployment)}
        deployment={deployment}
        onSave={onSave}
        isSaving={isSaving}
      />
    </SettingsPage>
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
