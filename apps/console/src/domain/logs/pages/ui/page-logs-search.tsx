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
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Database, Play, Search, ServerOff } from 'lucide-react'
import { NEW_SIGNATURE_HINT, summarizeSignatures, whyGroupFailed } from '../../grouping'
import { FacetPanel, type FacetRow } from './facet-panel'
import { LEVEL_FILL } from '../../level-tokens'
import { LogTable } from './log-table'
import { LogsHistogram } from './logs-histogram'
import {
  SEARCH_LEVELS,
  SEARCH_LEVEL_LABELS,
  SEARCH_WINDOWS,
  describeResults,
  facetShares,
  levelBreakdown,
  shortId,
  whySearchFailed,
  type FacetBucket,
  type SearchLevel,
} from '../../search'

/** Raw lines, or the same search collapsed into signatures (V4, #297). */
export type ResultView = 'lines' | 'signatures'

/** The three fields this screen breaks results down by -- fixed, since the
 * index's own doc mapping is fixed (see `docker/quickwit/index-config.template.yaml`),
 * not an open set of attributes a reader could pin arbitrarily. */
type FacetKey = 'level' | 'source' | 'deployment'

const FACET_TITLES: Record<FacetKey, string> = {
  level: 'Levels',
  source: 'Source',
  deployment: 'Deployment',
}

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
  /** The deployment's own actions, read back from the audit trail (#298). */
  actions: Schemas.Action[]
  windowFrom: number
  windowTo: number
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

      <span className='min-w-0 flex-1 truncate font-mono' title={signature.sample_message}>
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

/** The Levels facet's rows, always in severity order rather than by count. */
function levelRows(buckets: FacetBucket[]): FacetRow[] {
  return levelBreakdown(buckets).map((segment) => ({
    key: segment.level,
    label: <span className='uppercase'>{segment.level}</span>,
    count: segment.count,
    percent: segment.percent,
  }))
}

/** A facet's rows in count order, each carrying its own share of that facet. */
function attributeRows(buckets: FacetBucket[], render?: (value: string) => React.ReactNode): FacetRow[] {
  return facetShares(buckets).map((share) => ({
    key: share.value,
    label: render ? render(share.value) : share.value,
    count: share.count,
    percent: share.percent,
  }))
}

/**
 * A compact stacked bar of the level facet, folded into the Levels panel
 * itself rather than drawn a second time next to `LogsHistogram` -- the
 * histogram is already the real frequency-over-time chart (V5, #306); this
 * bar answers a different question ("which levels, in what share") that the
 * time axis doesn't.
 */
function LevelMiniBar({ buckets }: { buckets: FacetBucket[] }) {
  const segments = levelBreakdown(buckets)
  if (segments.length === 0) return null

  return (
    <div className='mb-1 flex h-2 w-full overflow-hidden rounded-full border bg-muted/20'>
      {segments.map((segment) => (
        <div
          key={segment.level}
          className={LEVEL_FILL[segment.level]}
          style={{ width: `${segment.percent}%` }}
          title={`${segment.level}: ${segment.count} (${Math.round(segment.percent)}%)`}
        />
      ))}
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
  actions,
  windowFrom,
  windowTo,
}: Props) {
  const refused = view === 'lines' ? errorStatus !== null : groupErrorStatus !== null

  const [pinned, setPinned] = useState<Set<FacetKey>>(new Set())
  const togglePin = (key: FacetKey) =>
    setPinned((current) => {
      const next = new Set(current)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })

  const facets: { key: FacetKey; rows: FacetRow[]; header?: React.ReactNode }[] = (
    [
      {
        key: 'level',
        rows: levelRows(result?.facets.level ?? []),
        header: <LevelMiniBar buckets={result?.facets.level ?? []} />,
      },
      { key: 'source', rows: attributeRows(result?.facets.source ?? []) },
      { key: 'deployment', rows: attributeRows(result?.facets.deployment_id ?? [], shortId) },
    ] satisfies { key: FacetKey; rows: FacetRow[]; header?: React.ReactNode }[]
  ).sort((a, b) => Number(pinned.has(b.key)) - Number(pinned.has(a.key)))

  return (
    <div className='flex flex-col gap-3'>
      <p className='flex items-start gap-2 rounded-md border bg-muted/30 px-3 py-2 text-sm text-muted-foreground'>
        <Database className='mt-0.5 h-4 w-4 shrink-0' aria-hidden />
        <span>
          Search looks at a separate, <strong className='font-medium text-foreground'>stored</strong>{' '}
          copy of your logs, kept for 30 days on the search index -- unlike the Live tab, which is
          never stored anywhere. Every search you run here is recorded in your audit log.
        </span>
      </p>

      {!refused && result && (
        <LogsHistogram
          buckets={result.buckets}
          actions={actions}
          from={windowFrom}
          to={windowTo}
        />
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
        <aside className='flex w-full shrink-0 flex-col gap-3 lg:w-60'>
          {facets.map((facet, index) => (
            <div key={facet.key} className='flex flex-col gap-3'>
              {index > 0 && <Separator />}
              <FacetPanel
                title={FACET_TITLES[facet.key]}
                rows={facet.rows}
                header={facet.header}
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
                  <LogTable hits={result.hits} />
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
                <div className='py-2'>
                  {groupResult.signatures.map((signature) => (
                    <Signature key={signature.fingerprint} signature={signature} />
                  ))}
                </div>
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
