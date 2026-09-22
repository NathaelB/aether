import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Separator } from '@/components/ui/separator'
import { Skeleton } from '@/components/ui/skeleton'
import { Button } from '@/components/ui/button'
import { Database, Play, Search, ServerOff } from 'lucide-react'
// Shared with the logs domain rather than duplicated: a facet's shape --
// count, share of the field's own total, an optional pin -- has nothing
// logs-specific about it. See `logs/pages/ui/facet-panel.tsx`.
import { FacetPanel, type FacetRow } from '../../../logs/pages/ui/facet-panel'
import { LogsHistogram } from '../../../logs/pages/ui/logs-histogram'
import { SEARCH_WINDOWS, describeResults, whySearchFailed } from '../../search'
import { TraceTable } from './trace-table'

type FacetKey = 'service' | 'status'

const FACET_TITLES: Record<FacetKey, string> = {
  service: 'Service',
  status: 'Status',
}

function facetRows(buckets: Schemas.TraceFacetBucket[]): FacetRow[] {
  const ordered = [...buckets].sort((a, b) => b.count - a.count || a.value.localeCompare(b.value))
  const total = ordered.reduce((sum, bucket) => sum + bucket.count, 0)

  return ordered.map((bucket) => ({
    key: bucket.value,
    label: bucket.value,
    count: bucket.count,
    percent: total === 0 ? 0 : (bucket.count / total) * 100,
  }))
}

interface Props {
  scopeLabel: string
  organisationId: string | null
  windowMinutes: number
  onWindowChange: (minutes: number) => void
  serviceName: string
  onServiceNameChange: (value: string) => void
  statusCode: string
  onStatusCodeChange: (value: string) => void
  text: string
  onTextChange: (text: string) => void
  onRun: () => void
  result?: Schemas.TraceSearchResult
  isLoading: boolean
  elapsedMs: number | null
  errorStatus: number | null
  windowFrom: number
  windowTo: number
}

export function PageTracesSearch({
  scopeLabel,
  organisationId,
  windowMinutes,
  onWindowChange,
  serviceName,
  onServiceNameChange,
  statusCode,
  onStatusCodeChange,
  text,
  onTextChange,
  onRun,
  result,
  isLoading,
  elapsedMs,
  errorStatus,
  windowFrom,
  windowTo,
}: Props) {
  const refused = errorStatus !== null
  const [pinned, setPinned] = useState<Set<FacetKey>>(new Set())
  const togglePin = (key: FacetKey) =>
    setPinned((current) => {
      const next = new Set(current)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })

  const facets: { key: FacetKey; rows: FacetRow[] }[] = (
    [
      { key: 'service', rows: facetRows(result?.facets.service_name ?? []) },
      { key: 'status', rows: facetRows(result?.facets.status_code ?? []) },
    ] satisfies { key: FacetKey; rows: FacetRow[] }[]
  ).sort((a, b) => Number(pinned.has(b.key)) - Number(pinned.has(a.key)))

  return (
    <div className='flex flex-col gap-3'>
      <p className='flex items-start gap-2 rounded-md border bg-muted/30 px-3 py-2 text-sm text-muted-foreground'>
        <Database className='mt-0.5 h-4 w-4 shrink-0' aria-hidden />
        <span>
          Search looks at this deployment's <strong className='font-medium text-foreground'>stored</strong>{' '}
          traces, kept for 30 days on the search index. Every search you run here is recorded in
          your audit log.
        </span>
      </p>

      {!refused && result && (
        <LogsHistogram buckets={result.buckets} actions={[]} from={windowFrom} to={windowTo} />
      )}

      <div className='flex flex-wrap items-center gap-2'>
        <span
          className='inline-flex h-8 shrink-0 items-center rounded-md border bg-muted/40 px-3 text-sm text-muted-foreground'
          title='Search is scoped to this deployment'
        >
          {scopeLabel}
        </span>

        <div className='relative min-w-56 flex-1'>
          <Search className='pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground' />
          <Input
            value={text}
            onChange={(event) => onTextChange(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') onRun()
            }}
            placeholder='Search span names…'
            className='h-8 pl-8 text-sm'
            aria-label='Search traces'
          />
        </div>

        <Select value={String(windowMinutes)} onValueChange={(value) => onWindowChange(Number(value))}>
          <SelectTrigger className='h-8 w-40 text-sm' aria-label='Time range'>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {SEARCH_WINDOWS.map(({ minutes, label }) => (
              <SelectItem key={minutes} value={String(minutes)}>
                {label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Button variant='outline' size='sm' onClick={onRun}>
          <Play className='h-3.5 w-3.5' />
          Run
        </Button>
      </div>

      {(serviceName !== '' || statusCode !== '') && (
        <div className='flex items-center gap-2 text-xs text-muted-foreground'>
          Filtered to
          {serviceName !== '' && (
            <button
              className='rounded border bg-muted/40 px-1.5 py-0.5 text-foreground hover:bg-muted/60'
              onClick={() => onServiceNameChange('')}
            >
              service: {serviceName} ×
            </button>
          )}
          {statusCode !== '' && (
            <button
              className='rounded border bg-muted/40 px-1.5 py-0.5 text-foreground hover:bg-muted/60'
              onClick={() => onStatusCodeChange('')}
            >
              status: {statusCode} ×
            </button>
          )}
        </div>
      )}

      <div className='flex flex-col gap-4 lg:flex-row'>
        <aside className='flex w-full shrink-0 flex-col gap-3 lg:w-60'>
          {facets.map((facet, index) => (
            <div key={facet.key} className='flex flex-col gap-3'>
              {index > 0 && <Separator />}
              <FacetPanel
                title={FACET_TITLES[facet.key]}
                rows={facet.rows.map((row) => ({
                  ...row,
                  label: (
                    <button
                      className='truncate text-left hover:underline'
                      onClick={() =>
                        facet.key === 'service'
                          ? onServiceNameChange(row.key)
                          : onStatusCodeChange(row.key)
                      }
                    >
                      {row.label}
                    </button>
                  ),
                }))}
                pinned={pinned.has(facet.key)}
                onTogglePin={() => togglePin(facet.key)}
                emptyLabel='No results to break down yet.'
              />
            </div>
          ))}
        </aside>

        <div className='flex min-w-0 flex-1 flex-col gap-3'>
          <div className='relative'>
            <div className='h-[calc(100svh-30rem)] min-h-72 overflow-auto rounded-lg border bg-card text-xs'>
              {refused ? (
                <div className='flex h-full flex-col items-center justify-center gap-2 px-6 text-center'>
                  <ServerOff className='h-6 w-6 text-muted-foreground' aria-hidden />
                  <p className='max-w-md text-sm text-muted-foreground'>
                    {whySearchFailed(errorStatus ?? 0)}
                  </p>
                </div>
              ) : isLoading ? (
                <div className='space-y-2 px-3 py-2'>
                  <Skeleton className='h-4 w-full' />
                  <Skeleton className='h-4 w-5/6' />
                  <Skeleton className='h-4 w-2/3' />
                </div>
              ) : !result || result.hits.length === 0 ? (
                <p className='px-3 py-2 text-muted-foreground'>
                  Nothing on the index matches. Widen the time range or clear the search text.
                </p>
              ) : (
                <TraceTable hits={result.hits} organisationId={organisationId} />
              )}
            </div>
          </div>

          {result && !refused && (
            <p className='flex items-center justify-end text-xs text-muted-foreground'>
              <span className='tabular-nums'>
                {describeResults(result.total_hits, result.hits.length, elapsedMs ?? 0)}
              </span>
            </p>
          )}
        </div>
      </div>
    </div>
  )
}
