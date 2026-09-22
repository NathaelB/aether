import type { Schemas } from '@/api/api.client'
import { cn } from '@/lib/utils'
import { formatDuration } from '../../search'
import { layoutWaterfall } from '../../waterfall'

const STATUS_CLASSES: Record<string, string> = {
  ok: 'bg-emerald-500',
  error: 'bg-red-500',
  unset: 'bg-muted-foreground/50',
}

/**
 * A trace's spans, laid out by `layoutWaterfall` and drawn as one bar per
 * row -- the simplest waterfall that still shows what a first cut needs:
 * which span called which, and how long each took relative to the trace.
 */
export function TraceWaterfall({ spans }: { spans: Schemas.SpanHit[] }) {
  const rows = layoutWaterfall(spans)

  if (rows.length === 0) {
    return <p className='px-3 py-2 text-xs text-muted-foreground'>This trace has no spans.</p>
  }

  return (
    <div className='flex flex-col gap-1 px-3 py-2'>
      {rows.map((row) => (
        <div key={row.span.span_id} className='flex items-center gap-2 text-xs'>
          <span
            className='w-56 shrink-0 truncate text-muted-foreground'
            style={{ paddingLeft: `${row.depth * 12}px` }}
            title={`${row.span.service_name} · ${row.span.name}`}
          >
            <span className='text-foreground'>{row.span.name}</span>{' '}
            <span className='text-muted-foreground/70'>({row.span.service_name})</span>
          </span>

          <div className='relative h-4 min-w-0 flex-1 rounded bg-muted/30'>
            <div
              className={cn(
                'absolute h-4 rounded',
                STATUS_CLASSES[row.span.status_code] ?? STATUS_CLASSES.unset,
              )}
              style={{ left: `${row.offsetPercent}%`, width: `${row.widthPercent}%` }}
              title={`${row.span.name}: ${formatDuration(row.span.duration_nanos)}`}
            />
          </div>

          <span className='w-16 shrink-0 text-right tabular-nums text-muted-foreground'>
            {formatDuration(row.span.duration_nanos)}
          </span>
        </div>
      ))}
    </div>
  )
}
