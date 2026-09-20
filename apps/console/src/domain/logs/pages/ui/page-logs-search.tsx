import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
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
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { cn } from '@/lib/utils'
import { ChevronDown, Database, Play, Search, ServerOff } from 'lucide-react'
import { NEW_SIGNATURE_HINT, summarizeSignatures, whyGroupFailed } from '../../grouping'
import {
  SEARCH_LEVELS,
  SEARCH_LEVEL_LABELS,
  SEARCH_WINDOWS,
  asResultLevel,
  describeResults,
  formatTimestamp,
  levelBreakdown,
  orderFacet,
  shortId,
  whySearchFailed,
  type FacetBucket,
  type ResultLevel,
  type SearchLevel,
} from '../../search'
import { toneFor } from '../../view'

/** Raw lines, or the same search collapsed into signatures (V4, #297). */
export type ResultView = 'lines' | 'signatures'

interface Props {
  scopeLabel: string
  windowMinutes: number
  onWindowChange: (minutes: number) => void
  floor: SearchLevel
  onFloorChange: (floor: SearchLevel) => void
  text: string
  onTextChange: (text: string) => void
  onRun: () => void
  view: ResultView
  onViewChange: (view: ResultView) => void
  result?: Schemas.LogSearchResult
  isLoading: boolean
  elapsedMs: number | null
  /** `null` outside a 409/403/404/400/… refusal -- a query still in flight or one that answered. */
  errorStatus: number | null
  groupResult?: Schemas.LogGroupResult
  isGrouping: boolean
  groupErrorStatus: number | null
}

const TONE_CLASSES = [
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
const LEVEL_CLASSES: Record<ResultLevel, string> = {
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
const LEVEL_FILL: Record<ResultLevel, string> = {
  trace: 'bg-muted-foreground/40',
  debug: 'bg-muted-foreground',
  info: 'bg-sky-500',
  warn: 'bg-amber-500',
  error: 'bg-red-500',
  fatal: 'bg-red-700',
  unknown: 'bg-muted-foreground/60',
}

function Hit({ hit }: { hit: Schemas.LogSearchHit }) {
  const level = asResultLevel(hit.level)

  return (
    <div className='flex gap-3 px-3 py-[3px] hover:bg-foreground/[0.04]'>
      <time className='w-40 shrink-0 tabular-nums text-muted-foreground' dateTime={hit.timestamp}>
        {formatTimestamp(hit.timestamp)}
      </time>

      <span
        className={cn('w-14 shrink-0 text-right text-[10px] uppercase leading-4', LEVEL_CLASSES[level])}
      >
        {level}
      </span>

      <span
        className={cn('w-28 shrink-0 truncate', TONE_CLASSES[toneFor(hit.source)])}
        title={hit.source}
      >
        {hit.source}
      </span>

      <span
        className='w-20 shrink-0 truncate text-muted-foreground/70'
        title={hit.deployment_id}
      >
        {shortId(hit.deployment_id)}
      </span>

      <span className={cn('min-w-0 flex-1 truncate', LEVEL_CLASSES[level])} title={hit.message}>
        {hit.message}
      </span>
    </div>
  )
}

/** One fingerprint signature: a burst of identical lines collapsed to a count (V4, #297). */
function Signature({ signature }: { signature: Schemas.LogSignature }) {
  return (
    <div className='flex items-center gap-3 px-3 py-[3px] hover:bg-foreground/[0.04]'>
      <span className='w-12 shrink-0 text-right tabular-nums text-muted-foreground'>
        ×{signature.count}
      </span>

      {signature.is_new ? (
        <span
          title={NEW_SIGNATURE_HINT}
          className='shrink-0 rounded border border-primary/30 bg-primary/10 px-1 py-0 text-[10px] font-medium uppercase leading-4 text-primary'
        >
          New
        </span>
      ) : (
        <span className='w-9 shrink-0' />
      )}

      <span className='min-w-0 flex-1 truncate' title={signature.sample_message}>
        {signature.sample_message}
      </span>

      <span
        className='hidden shrink-0 font-mono text-muted-foreground/60 sm:inline'
        title='Fingerprint'
      >
        {signature.fingerprint}
      </span>
    </div>
  )
}

/** The Levels facet: every level the search covers, always in severity order. */
function LevelFacet({ buckets }: { buckets: FacetBucket[] }) {
  const segments = levelBreakdown(buckets)

  if (segments.length === 0) {
    return <p className='text-xs text-muted-foreground'>No results to break down yet.</p>
  }

  return (
    <ul className='flex flex-col gap-1'>
      {segments.map((segment) => (
        <li
          key={segment.level}
          className='flex items-center justify-between gap-2 text-xs'
        >
          <span className={cn('uppercase', LEVEL_CLASSES[segment.level])}>{segment.level}</span>
          <span className='tabular-nums text-muted-foreground'>{segment.count}</span>
        </li>
      ))}
    </ul>
  )
}

/** A collapsible facet for an attribute that is not a severity -- source, deployment. */
function AttributeFacet({
  title,
  buckets,
  render,
}: {
  title: string
  buckets: FacetBucket[]
  render?: (value: string) => React.ReactNode
}) {
  const [open, setOpen] = useState(true)
  const ordered = orderFacet(buckets)

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger className='flex w-full items-center justify-between text-xs font-medium text-muted-foreground hover:text-foreground'>
        {title}
        <ChevronDown className={cn('h-3.5 w-3.5 transition-transform', open && 'rotate-180')} />
      </CollapsibleTrigger>
      <CollapsibleContent className='mt-2 flex flex-col gap-1'>
        {ordered.length === 0 ? (
          <p className='text-xs text-muted-foreground'>Nothing yet.</p>
        ) : (
          ordered.map((bucket) => (
            <div key={bucket.value} className='flex items-center justify-between gap-2 text-xs'>
              <span className='truncate text-foreground' title={bucket.value}>
                {render ? render(bucket.value) : bucket.value}
              </span>
              <span className='shrink-0 tabular-nums text-muted-foreground'>{bucket.count}</span>
            </div>
          ))
        )}
      </CollapsibleContent>
    </Collapsible>
  )
}

/**
 * A stacked bar of the level facet, standing in for a frequency histogram.
 *
 * There is no `date_histogram` endpoint yet -- that lands with V5 -- and
 * bucketing the 200 hits this screen can see by time would draw a shape that
 * is an artefact of the cap, not of what actually happened. This bar is
 * honest about what it is: a split of matches by level, not over time.
 */
function LevelBar({ buckets }: { buckets: FacetBucket[] }) {
  const segments = levelBreakdown(buckets)
  const total = segments.reduce((sum, segment) => sum + segment.count, 0)

  return (
    <div className='flex flex-col gap-1.5'>
      <div className='flex h-6 w-full overflow-hidden rounded-md border bg-muted/20'>
        {total === 0 ? null : (
          segments.map((segment) => (
            <div
              key={segment.level}
              className={LEVEL_FILL[segment.level]}
              style={{ width: `${segment.percent}%` }}
              title={`${segment.level}: ${segment.count}`}
            />
          ))
        )}
      </div>
      <p className='text-[11px] text-muted-foreground'>
        Split by level across the matches shown here, not a frequency over time — a real
        time-bucketed histogram needs the search index's own date histogram, which is not yet
        available on this installation.
      </p>
    </div>
  )
}

export function PageLogsSearch({
  scopeLabel,
  windowMinutes,
  onWindowChange,
  floor,
  onFloorChange,
  text,
  onTextChange,
  onRun,
  view,
  onViewChange,
  result,
  isLoading,
  elapsedMs,
  errorStatus,
  groupResult,
  isGrouping,
  groupErrorStatus,
}: Props) {
  const refused = view === 'lines' ? errorStatus !== null : groupErrorStatus !== null

  return (
    <div className='mt-6 flex flex-col gap-3'>
      <p className='flex items-start gap-2 rounded-md border bg-muted/30 px-3 py-2 text-sm text-muted-foreground'>
        <Database className='mt-0.5 h-4 w-4 shrink-0' aria-hidden />
        <span>
          Search looks at a separate, <strong className='font-medium text-foreground'>stored</strong>{' '}
          copy of your logs, kept for 30 days on the search index -- unlike the Live tab, which is
          never stored anywhere. Every search you run here is recorded in your audit log.
        </span>
      </p>

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
            placeholder='Search logs…'
            className='h-8 pl-8 text-sm'
            aria-label='Search logs'
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

        <Select value={floor} onValueChange={(value) => onFloorChange(value as SearchLevel)}>
          <SelectTrigger className='h-8 w-52 text-sm' aria-label='Least severe level to include'>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {SEARCH_LEVELS.map((level) => (
              <SelectItem key={level} value={level}>
                {SEARCH_LEVEL_LABELS[level]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Button variant='outline' size='sm' onClick={onRun}>
          <Play className='h-3.5 w-3.5' />
          Run
        </Button>

        <Tabs value={view} onValueChange={(value) => onViewChange(value as ResultView)}>
          <TabsList className='h-8 p-0.5'>
            <TabsTrigger value='lines' className='px-2.5 py-1 text-xs'>
              Lines
            </TabsTrigger>
            <TabsTrigger value='signatures' className='px-2.5 py-1 text-xs'>
              Signatures
            </TabsTrigger>
          </TabsList>
        </Tabs>
      </div>

      <p className='text-xs text-muted-foreground'>
        Lines the platform could not classify are always included, whatever the level above says.
      </p>

      <div className='flex flex-col gap-4 lg:flex-row'>
        <aside className='flex w-full shrink-0 flex-col gap-4 lg:w-56'>
          <div>
            <h3 className='mb-2 text-xs font-medium text-muted-foreground'>Levels</h3>
            <LevelFacet buckets={result?.facets.level ?? []} />
          </div>

          <Separator />

          <AttributeFacet title='Source' buckets={result?.facets.source ?? []} />

          <Separator />

          <AttributeFacet
            title='Deployment'
            buckets={result?.facets.deployment_id ?? []}
            render={shortId}
          />
        </aside>

        <div className='flex min-w-0 flex-1 flex-col gap-3'>
          <LevelBar buckets={result?.facets.level ?? []} />

          <div className='relative'>
            <div className='h-[calc(100svh-30rem)] min-h-72 overflow-auto rounded-lg border bg-muted/20 py-2 font-mono text-xs'>
              {refused ? (
                <div className='flex h-full flex-col items-center justify-center gap-2 px-6 text-center'>
                  <ServerOff className='h-6 w-6 text-muted-foreground' aria-hidden />
                  <p className='max-w-md text-sm text-muted-foreground'>
                    {view === 'lines'
                      ? whySearchFailed(errorStatus ?? 0)
                      : whyGroupFailed(groupErrorStatus ?? 0)}
                  </p>
                </div>
              ) : view === 'lines' ? (
                isLoading ? (
                  <div className='space-y-2 px-3 py-2'>
                    <Skeleton className='h-4 w-full' />
                    <Skeleton className='h-4 w-5/6' />
                    <Skeleton className='h-4 w-2/3' />
                  </div>
                ) : !result || result.hits.length === 0 ? (
                  <p className='px-3 py-2 text-muted-foreground'>
                    Nothing on the index matches. Widen the time range, lower the level, or clear the
                    search text.
                  </p>
                ) : (
                  result.hits.map((hit, index) => <Hit key={`${hit.timestamp}-${index}`} hit={hit} />)
                )
              ) : isGrouping ? (
                <div className='space-y-2 px-3 py-2'>
                  <Skeleton className='h-4 w-full' />
                  <Skeleton className='h-4 w-5/6' />
                  <Skeleton className='h-4 w-2/3' />
                </div>
              ) : !groupResult || groupResult.signatures.length === 0 ? (
                <p className='px-3 py-2 text-muted-foreground'>
                  Nothing on the index matches. Widen the time range, lower the level, or clear the
                  search text.
                </p>
              ) : (
                groupResult.signatures.map((signature) => (
                  <Signature key={signature.fingerprint} signature={signature} />
                ))
              )}
            </div>
          </div>

          {view === 'lines'
            ? result &&
              !refused && (
                <p className='flex items-center justify-end text-xs text-muted-foreground'>
                  <span className='tabular-nums'>
                    {describeResults(result.total_hits, result.hits.length, elapsedMs ?? 0)}
                  </span>
                </p>
              )
            : groupResult &&
              !refused && (
                <p className='flex items-center justify-end text-xs text-muted-foreground'>
                  <span className='tabular-nums'>{summarizeSignatures(groupResult.signatures)}</span>
                </p>
              )}
        </div>
      </div>
    </div>
  )
}
