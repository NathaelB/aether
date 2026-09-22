import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import { cn } from '@/lib/utils'
import { ChevronRight } from 'lucide-react'
import { LEVEL_CLASSES, TONE_CLASSES } from '../../level-tokens'
import { asResultLevel, formatTimestamp } from '../../search'
import { toneFor } from '../../view'

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
