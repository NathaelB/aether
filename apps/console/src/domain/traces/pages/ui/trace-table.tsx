import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import { useGetTrace } from '@/api/traces.api'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'
import { ChevronRight } from 'lucide-react'
import { formatDuration, shortTraceId } from '../../search'
import { TraceWaterfall } from './trace-waterfall'

const STATUS_CLASSES: Record<string, string> = {
  ok: 'text-emerald-600 dark:text-emerald-400',
  error: 'text-red-600 dark:text-red-400',
  unset: 'text-muted-foreground',
}

function Row({ hit, organisationId }: { hit: Schemas.SpanHit; organisationId: string | null }) {
  const [expanded, setExpanded] = useState(false)
  const trace = useGetTrace(organisationId, hit.trace_id, expanded)

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
        <td className='w-32 truncate py-1 pr-3 align-top font-mono text-muted-foreground' title={hit.trace_id}>
          {shortTraceId(hit.trace_id)}
        </td>
        <td className='w-32 truncate py-1 pr-3 align-top' title={hit.service_name}>
          {hit.service_name}
        </td>
        <td className='min-w-0 truncate py-1 pr-3 align-top' title={hit.name}>
          {hit.name}
        </td>
        <td className='w-20 shrink-0 py-1 pr-3 text-right align-top tabular-nums'>
          {formatDuration(hit.duration_nanos)}
        </td>
        <td
          className={cn(
            'w-16 shrink-0 py-1 pr-2 text-right align-top uppercase',
            STATUS_CLASSES[hit.status_code] ?? STATUS_CLASSES.unset,
          )}
        >
          {hit.status_code}
        </td>
      </tr>

      {expanded && (
        <tr className='border-b border-border/40 bg-muted/20'>
          <td colSpan={6}>
            {trace.isLoading ? (
              <div className='space-y-2 px-3 py-2'>
                <Skeleton className='h-4 w-full' />
                <Skeleton className='h-4 w-2/3' />
              </div>
            ) : (
              <TraceWaterfall spans={trace.data?.spans ?? []} />
            )}
          </td>
        </tr>
      )}
    </>
  )
}

/**
 * The Lines view's counterpart for traces: a structured table with a
 * `trace_id | service | operation | duration | status` header, whose rows
 * expand in place into that trace's own waterfall rather than opening a
 * separate page.
 */
export function TraceTable({
  hits,
  organisationId,
}: {
  hits: Schemas.SpanHit[]
  organisationId: string | null
}) {
  return (
    <table className='w-full min-w-0 table-fixed border-collapse text-xs'>
      <thead className='sticky top-0 z-10 bg-background'>
        <tr className='border-b text-left text-[11px] uppercase tracking-wide text-muted-foreground'>
          <th className='w-5 py-1.5 pl-2 font-medium' />
          <th className='w-32 py-1.5 pr-3 font-medium'>Trace</th>
          <th className='w-32 py-1.5 pr-3 font-medium'>Service</th>
          <th className='py-1.5 pr-3 font-medium'>Operation</th>
          <th className='w-20 py-1.5 pr-3 text-right font-medium'>Duration</th>
          <th className='w-16 py-1.5 pr-2 text-right font-medium'>Status</th>
        </tr>
      </thead>
      <tbody>
        {hits.map((hit, index) => (
          <Row key={`${hit.trace_id}-${hit.span_id}-${index}`} hit={hit} organisationId={organisationId} />
        ))}
      </tbody>
    </table>
  )
}
