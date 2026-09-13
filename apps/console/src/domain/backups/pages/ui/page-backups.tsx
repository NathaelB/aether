import { useState } from 'react'
import { Archive, Lock, ShieldCheck } from 'lucide-react'
import { EmptyState, Section, SettingsPage } from '@/components/layout/page'
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
import { Spinner } from '@/components/ui/spinner'
import { StatusBadge } from '@/components/ui/status-badge'
import { Switch } from '@/components/ui/switch'
import {
  describeProtection,
  newestFirst,
  readableAge,
  readableDuration,
  readableSize,
  summarise,
} from '../../archives'
import {
  WEEKDAYS,
  checkForm,
  describe,
  formFor,
  hasChanges,
  requestFrom,
  type Backup,
  type BackupSchedule,
  type ScheduleForm,
  type ScheduleRequest,
  type Weekday,
} from '../../schedule'

interface Props {
  backups?: Backup[]
  schedule?: BackupSchedule
  isLoading: boolean
  isSaving: boolean
  mayManage: boolean
  onSave: (request: ScheduleRequest) => void
}

export function PageBackups({
  backups,
  schedule,
  isLoading,
  isSaving,
  mayManage,
  onSave,
}: Props) {
  /// Edits belong to the version they were made against.
  ///
  /// Derived rather than copied in on arrival: a draft carried over a schedule
  /// the platform has since answered with would go on showing a value that was
  /// adjusted or refused, and the screen would disagree with the instance
  /// while looking settled. Saving moves `updated_at`, which is what drops the
  /// draft and puts the stored schedule back on screen.
  const [draft, setDraft] = useState<{ of: string; form: ScheduleForm } | null>(null)

  if (isLoading || !schedule) {
    return <Skeleton className='h-64 w-full' />
  }

  const form = draft?.of === schedule.updated_at ? draft.form : formFor(schedule)
  const problem = checkForm(form)
  const changed = hasChanges(form, schedule)
  const archives = newestFirst(backups ?? [])
  const edit = (change: Partial<ScheduleForm>) =>
    setDraft({ of: schedule.updated_at, form: { ...form, ...change } })

  return (
    <SettingsPage
      title='Backups'
      description='When this instance is archived, and what has been archived so far.'
    >
      <Section
        title='Schedule'
        aside={
          mayManage ? (
            <Button
              size='sm'
              disabled={!changed || !!problem || isSaving}
              onClick={() => onSave(requestFrom(form, schedule.zone))}
            >
              {isSaving ? <Spinner className='h-3.5 w-3.5' /> : null}
              Save
            </Button>
          ) : undefined
        }
      >
        <div className='rounded-lg border'>
          <div className='flex items-center gap-2 border-b bg-muted/30 px-4 py-2.5'>
            <StatusBadge tone={schedule.enabled ? 'success' : 'warning'} dot={false}>
              {schedule.enabled ? 'On' : 'Off'}
            </StatusBadge>
            <span className='text-xs text-muted-foreground'>{describe(schedule)}</span>
          </div>

          <fieldset disabled={!mayManage} className='grid gap-5 p-4 sm:grid-cols-2'>
            <div className='flex items-center justify-between gap-4 sm:col-span-2'>
              <div>
                <Label htmlFor='backups-enabled'>Take backups</Label>
                <p className='text-xs text-muted-foreground'>
                  Turning them off keeps this schedule, so turning them back on does not mean
                  filling the form in again.
                </p>
              </div>
              <Switch
                id='backups-enabled'
                checked={form.enabled}
                onCheckedChange={(enabled) => edit({ enabled })}
              />
            </div>

            <div className='space-y-1.5'>
              <Label htmlFor='backups-every'>How often</Label>
              <Select
                value={form.every}
                onValueChange={(every) => edit({ every: every as ScheduleForm['every'] })}
              >
                <SelectTrigger id='backups-every'>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value='daily'>Every day</SelectItem>
                  <SelectItem value='weekly'>Every week</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className='space-y-1.5'>
              <Label htmlFor='backups-at'>At</Label>
              <div className='flex gap-2'>
                {form.every === 'weekly' ? (
                  <Select value={form.day} onValueChange={(day) => edit({ day: day as Weekday })}>
                    <SelectTrigger className='w-28'>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {WEEKDAYS.map((day) => (
                        <SelectItem key={day} value={day}>
                          {day}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ) : null}
                <Input
                  id='backups-at'
                  type='time'
                  value={form.at}
                  onChange={(event) => edit({ at: event.target.value })}
                />
              </div>
              <p className='text-xs text-muted-foreground'>
                Read in {schedule.zone}, so it stays {form.at} across a daylight saving change.
              </p>
            </div>

            <div className='space-y-1.5'>
              <Label htmlFor='backups-keep-last'>Always keep the last</Label>
              <Input
                id='backups-keep-last'
                type='number'
                min={1}
                value={form.keepLast}
                onChange={(event) => edit({ keepLast: event.target.value })}
              />
              <p className='text-xs text-muted-foreground'>
                Archives, whatever their age.
              </p>
            </div>

            <div className='space-y-1.5'>
              <Label htmlFor='backups-keep-days'>And everything from the last</Label>
              <Input
                id='backups-keep-days'
                type='number'
                min={0}
                value={form.keepForDays}
                onChange={(event) => edit({ keepForDays: event.target.value })}
              />
              <p className='text-xs text-muted-foreground'>
                Days, however many that is. Whichever rule keeps more wins.
              </p>
            </div>
          </fieldset>

          {problem ? (
            <p className='border-t px-4 py-2.5 text-xs text-destructive'>{problem}</p>
          ) : null}
        </div>
      </Section>

      <Section title='Archives'>
        <p className='mb-3 text-xs text-muted-foreground'>
          {summarise(archives, schedule.enabled)}
        </p>

        {archives.length === 0 ? (
          <EmptyState
            icon={<Archive className='h-5 w-5' />}
            title={schedule.enabled ? 'Nothing archived yet' : 'Nothing archived'}
            description={
              schedule.enabled
                ? 'The first archive is taken on the next scheduled run.'
                : 'Backups are off for this instance, so none is being taken.'
            }
          />
        ) : (
          <div className='overflow-hidden rounded-lg border'>
            <ul>
              {archives.map((archive) => (
                <li
                  key={archive.id}
                  className='flex flex-wrap items-center justify-between gap-x-6 gap-y-1 border-b px-4 py-3 last:border-b-0'
                >
                  <div className='min-w-0'>
                    <p className='text-sm'>{readableAge(archive.finished_at)}</p>
                    <p className='truncate text-xs text-muted-foreground'>
                      {new Date(archive.finished_at).toLocaleString()} &middot; took{' '}
                      {readableDuration(archive.started_at, archive.finished_at)}
                    </p>
                  </div>
                  <div className='flex items-center gap-4 text-xs text-muted-foreground'>
                    <span title={describeProtection(archive)}>
                      {archive.protection.kind === 'envelope' ? (
                        <Lock className='h-3.5 w-3.5' />
                      ) : (
                        <ShieldCheck className='h-3.5 w-3.5' />
                      )}
                    </span>
                    <span>Postgres {archive.postgres_major}</span>
                    <span className='tabular-nums'>{readableSize(archive.size_bytes)}</span>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}
      </Section>
    </SettingsPage>
  )
}
