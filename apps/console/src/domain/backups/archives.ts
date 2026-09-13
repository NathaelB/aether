import type { Backup } from './schedule'

/**
 * An archive's size, in the units somebody reads rather than the ones it was
 * measured in.
 *
 * The platform sends bytes because that is what was measured, and a
 * three gigabyte archive shown as 3221225472 tells a reader nothing they came
 * for.
 */
export function readableSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '—'
  if (bytes < 1024) return `${bytes} B`

  const units = ['kB', 'MB', 'GB', 'TB']
  let value = bytes / 1024
  let unit = 0

  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }

  // One decimal below ten, none above. "1.4 GB" is a size; "1.4382 GB" is a
  // measurement, and the extra digits are noise on a screen somebody is
  // scanning.
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`
}

/** How long an archive took, in the units it actually took. */
export function readableDuration(startedAt: string, finishedAt: string): string {
  const seconds = Math.max(
    0,
    Math.round((new Date(finishedAt).getTime() - new Date(startedAt).getTime()) / 1000),
  )

  if (!Number.isFinite(seconds)) return '—'
  if (seconds < 60) return `${seconds}s`

  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ${seconds % 60}s`

  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`
}

/**
 * How long ago an archive finished.
 *
 * Relative, because the question a backup screen answers is "how old is the
 * newest one", and an absolute timestamp makes the reader do that subtraction
 * themselves.
 */
export function readableAge(finishedAt: string, now: number = Date.now()): string {
  const seconds = Math.round((now - new Date(finishedAt).getTime()) / 1000)

  if (!Number.isFinite(seconds)) return '—'
  if (seconds < 0) return 'just now'
  if (seconds < 60) return 'just now'

  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes} minute${minutes === 1 ? '' : 's'} ago`

  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} hour${hours === 1 ? '' : 's'} ago`

  const days = Math.floor(hours / 24)

  return `${days} day${days === 1 ? '' : 's'} ago`
}

/** Newest first, whatever order they arrived in. */
export function newestFirst(backups: Backup[]): Backup[] {
  return [...backups].sort(
    (one, other) => new Date(other.finished_at).getTime() - new Date(one.finished_at).getTime(),
  )
}

/**
 * What protects an archive, said the way a customer asks it: can this platform
 * read my data.
 */
export function describeProtection(backup: Backup): string {
  return backup.protection.kind === 'envelope'
    ? 'Encrypted before it left the cluster'
    : 'Encrypted by the object store'
}

/**
 * What to say above the list.
 *
 * Three states, and they are not the same. Nothing archived yet is a young
 * deployment; nothing archived with backups off is a decision somebody made;
 * and an archive that is older than the schedule says it should be is the one
 * worth noticing.
 */
export function summarise(
  backups: Backup[],
  enabled: boolean,
  now: number = Date.now(),
): string {
  if (backups.length === 0) {
    return enabled
      ? 'No archive yet. The first one is taken on the next scheduled run.'
      : 'No archive, and backups are off for this instance.'
  }

  const newest = newestFirst(backups)[0]

  return `${plural(backups.length, 'archive')}, the newest ${readableAge(
    newest.finished_at,
    now,
  )}.`
}

function plural(count: number, noun: string): string {
  return count === 1 ? `${count} ${noun}` : `${count} ${noun}s`
}
