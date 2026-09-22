import type { ResultLevel } from './search'

/** A stable colour per source, matching the live tail's own palette. */
export const TONE_CLASSES = [
  'text-sky-600 dark:text-sky-400',
  'text-emerald-600 dark:text-emerald-400',
  'text-violet-600 dark:text-violet-400',
  'text-amber-600 dark:text-amber-400',
  'text-rose-600 dark:text-rose-400',
  'text-teal-600 dark:text-teal-400',
] satisfies string[]

/**
 * Severity, drawn as severity -- one step darker than the live tail's own
 * palette at the top end, since `fatal` is a level the live tail never has to
 * draw.
 */
export const LEVEL_CLASSES: Record<ResultLevel, string> = {
  trace: 'text-muted-foreground/60',
  debug: 'text-muted-foreground',
  info: 'text-foreground',
  warn: 'text-amber-600 dark:text-amber-400',
  error: 'text-red-600 dark:text-red-400',
  fatal: 'font-semibold text-red-700 dark:text-red-300',
  /** Not a severity Herald observed -- a format it could not read. Told apart
   * rather than coloured as a guess at how bad the line was. */
  unknown: 'italic text-muted-foreground',
}

/** The bar segment for each level, matching `LEVEL_CLASSES` at the fill rather than the text. */
export const LEVEL_FILL: Record<ResultLevel, string> = {
  trace: 'bg-muted-foreground/40',
  debug: 'bg-muted-foreground',
  info: 'bg-sky-500',
  warn: 'bg-amber-500',
  error: 'bg-red-500',
  fatal: 'bg-red-700',
  unknown: 'bg-muted-foreground/60',
}
