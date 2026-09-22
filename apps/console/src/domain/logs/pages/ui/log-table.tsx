import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import { cn } from '@/lib/utils'
import { ChevronRight } from 'lucide-react'
import { asResultLevel, formatTimestamp, type ResultLevel } from '../../search'
import { toneFor } from '../../view'

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

function Row({ hit }: { hit: Schemas.LogSearchHit }) {
  const [expanded, setExpanded] = useState(false)
  const level = asResultLevel(hit.level)

  return (
    <>
      <tr
        className='cursor-pointer border-b border-border/40 hover:bg-foreground/[0.04]'
        onClick={() => setExpanded((value) => !value)}
        aria-expanded={expanded}
      >
        <td className='w-5 py-1 pl-2 align-top text-muted-foreground'>
          <ChevronRight className={cn('h-3.5 w-3.5 transition-transform', expanded && 'rotate-90')} />
        </td>
        <td className='w-40 whitespace-nowrap py-1 pr-3 align-top tabular-nums text-muted-foreground'>
          {formatTimestamp(hit.timestamp)}
        </td>
        <td
          className={cn('w-28 truncate py-1 pr-3 align-top', TONE_CLASSES[toneFor(hit.source)])}
          title={hit.source}
        >
          {hit.source}
        </td>
        <td className='min-w-0 py-1 pr-2 align-top'>
          <span className={cn('block truncate font-mono', LEVEL_CLASSES[level])} title={hit.message}>
            {hit.message}
          </span>
        </td>
      </tr>

      {expanded && (
        <tr className='border-b border-border/40 bg-muted/20'>
          <td colSpan={4} className='px-3 py-2'>
            <dl className='grid grid-cols-[auto_1fr] gap-x-3 gap-y-1'>
              <dt className='text-muted-foreground'>Level</dt>
              <dd className={cn('uppercase', LEVEL_CLASSES[level])}>{level}</dd>

              <dt className='text-muted-foreground'>Source</dt>
              <dd className={TONE_CLASSES[toneFor(hit.source)]}>{hit.source}</dd>

              <dt className='text-muted-foreground'>Deployment</dt>
              <dd className='font-mono'>{hit.deployment_id}</dd>

              <dt className='self-start text-muted-foreground'>Message</dt>
              <dd className='whitespace-pre-wrap break-words font-mono'>{hit.message}</dd>
            </dl>
          </td>
        </tr>
      )}
    </>
  )
}

/**
 * The Lines view, as a structured table rather than a flat run of text.
 *
 * A `timestamp | source | message` header, sticky within the scrolling
 * results panel; a row expands in place to the fields a truncated cell
 * hides, rather than opening a separate panel.
 */
export function LogTable({ hits }: { hits: Schemas.LogSearchHit[] }) {
  return (
    <table className='w-full min-w-0 table-fixed border-collapse text-xs'>
      <thead className='sticky top-0 z-10 bg-background'>
        <tr className='border-b text-left text-[11px] uppercase tracking-wide text-muted-foreground'>
          <th className='w-5 py-1.5 pl-2 font-medium' />
          <th className='w-40 py-1.5 pr-3 font-medium'>Timestamp</th>
          <th className='w-28 py-1.5 pr-3 font-medium'>Source</th>
          <th className='py-1.5 pr-2 font-medium'>Message</th>
        </tr>
      </thead>
      <tbody>
        {hits.map((hit, index) => (
          <Row key={`${hit.timestamp}-${index}`} hit={hit} />
        ))}
      </tbody>
    </table>
  )
}
