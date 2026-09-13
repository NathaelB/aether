import type { Schemas } from '@/api/api.client'

export type Backup = Schemas.Backup
export type BackupSchedule = Schemas.BackupSchedule

/**
 * The form's own shape, which is not the platform's.
 *
 * The platform reads a cadence as a sum -- daily, or weekly with a day -- and
 * a form has to hold both branches at once, because somebody switching to
 * weekly and back should not find the time they typed gone. Converting at the
 * edges is what keeps the sum honest on the side that has to reason about it.
 */
export interface ScheduleForm {
  every: 'daily' | 'weekly'
  at: string
  day: Weekday
  keepLast: string
  keepForDays: string
  enabled: boolean
}

export type Weekday = 'Mon' | 'Tue' | 'Wed' | 'Thu' | 'Fri' | 'Sat' | 'Sun'

export const WEEKDAYS: Weekday[] = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']

/** What the platform holds, as a form somebody can edit. */
export function formFor(schedule: BackupSchedule): ScheduleForm {
  const weekly = schedule.cadence.every === 'weekly' ? schedule.cadence : undefined

  return {
    every: schedule.cadence.every,
    at: hoursAndMinutes(schedule.cadence.at),
    // A default rather than nothing, so switching to weekly lands on a real
    // day instead of an empty select nobody can submit.
    day: (weekly?.day as Weekday) ?? 'Sun',
    keepLast: String(schedule.retention.keep_last),
    keepForDays: String(schedule.retention.keep_for_days),
    enabled: schedule.enabled,
  }
}

/**
 * The times the platform sends can carry seconds; the input cannot show them.
 *
 * Trimmed rather than reformatted, so a value this console did not produce
 * still reaches the field it belongs in.
 */
function hoursAndMinutes(at: string): string {
  return at.slice(0, 5)
}

export interface ScheduleRequest {
  every: string
  at: string
  day?: string
  zone: string
  keep_last: number
  keep_for_days: number
  enabled: boolean
}

/** What the form amounts to, as the API takes it. */
export function requestFrom(form: ScheduleForm, zone: string): ScheduleRequest {
  return {
    every: form.every,
    at: form.at,
    // Only when it means something. A day beside a daily cadence is a value
    // the platform ignores, and one the next reader has to work out is
    // ignored.
    ...(form.every === 'weekly' ? { day: form.day } : {}),
    zone,
    keep_last: Number(form.keepLast),
    keep_for_days: Number(form.keepForDays),
    enabled: form.enabled,
  }
}

/**
 * What is wrong with the form, or nothing.
 *
 * Said before the request rather than after the refusal. The platform
 * refuses these too -- it has to, since it does not trust a form -- but a
 * field that goes red as you leave it beats one that goes red after a round
 * trip.
 */
export function checkForm(form: ScheduleForm): string | null {
  if (!/^\d{2}:\d{2}$/.test(form.at)) return 'A time of day is needed, as HH:MM.'

  const keepLast = Number(form.keepLast)
  if (!Number.isInteger(keepLast) || keepLast < 1) {
    return 'Keeping zero archives is not a retention policy.'
  }

  const keepForDays = Number(form.keepForDays)
  if (!Number.isInteger(keepForDays) || keepForDays < 0) {
    return 'A window of days cannot run backwards.'
  }

  return null
}

/** Whether the form says anything the schedule does not already say. */
export function hasChanges(form: ScheduleForm, schedule: BackupSchedule): boolean {
  const saved = formFor(schedule)

  // The day only counts when the cadence uses it: switching to weekly and
  // back leaves a day behind that changes nothing.
  const compared = (value: ScheduleForm) => ({
    ...value,
    day: value.every === 'weekly' ? value.day : '',
  })

  return JSON.stringify(compared(form)) !== JSON.stringify(compared(saved))
}

/** What this schedule does, in one line. */
export function describe(schedule: BackupSchedule): string {
  if (!schedule.enabled) return 'Backups are off for this instance.'

  const at = hoursAndMinutes(schedule.cadence.at)
  const when =
    schedule.cadence.every === 'weekly'
      ? `every ${dayName(schedule.cadence.day)} at ${at}`
      : `every day at ${at}`

  return `Archived ${when} ${schedule.zone}, keeping the last ${plural(
    schedule.retention.keep_last,
    'archive',
  )} and anything from the last ${plural(schedule.retention.keep_for_days, 'day')}.`
}

const DAY_NAMES: Record<string, string> = {
  Mon: 'Monday',
  Tue: 'Tuesday',
  Wed: 'Wednesday',
  Thu: 'Thursday',
  Fri: 'Friday',
  Sat: 'Saturday',
  Sun: 'Sunday',
}

function dayName(day: string): string {
  return DAY_NAMES[day] ?? day
}

function plural(count: number, noun: string): string {
  return count === 1 ? `${count} ${noun}` : `${count} ${noun}s`
}
