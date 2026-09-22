/**
 * Asking what happened, rather than watching it happen.
 *
 * The live tail follows a socket; a search turns a time range, a level floor
 * and some free text into one request against a stored, 30-day index. The
 * shapes below are what stands between the three controls on screen and the
 * query the generated client sends -- kept here, and tested here, rather than
 * inside the component that renders them.
 */

export type SearchLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error' | 'fatal'

/**
 * Least severe first, matching the order the control plane itself defines
 * (`LogLevel` in `libs/aether-domain/src/logs/mod.rs`). `unknown` is
 * deliberately not one of these: it is not a floor a caller may ask for, and
 * offering it as a choice here would suggest otherwise.
 */
export const SEARCH_LEVELS: SearchLevel[] = ['trace', 'debug', 'info', 'warn', 'error', 'fatal']

export const SEARCH_LEVEL_LABELS: Record<SearchLevel, string> = {
  trace: 'Everything',
  debug: 'Debug and above',
  info: 'Info and above',
  warn: 'Warnings and above',
  error: 'Errors and above',
  fatal: 'Fatal only',
}

export const DEFAULT_SEARCH_LEVEL: SearchLevel = 'info'

/**
 * How far back a search may reach, offered rather than a free field: the
 * index keeps 30 days, and the widest preset here matches that cap exactly
 * rather than inventing a second number that could drift from it.
 */
export const SEARCH_WINDOWS = [
  { minutes: 15, label: 'Last 15 minutes' },
  { minutes: 60, label: 'Last hour' },
  { minutes: 60 * 24, label: 'Last 24 hours' },
  { minutes: 60 * 24 * 7, label: 'Last 7 days' },
  { minutes: 60 * 24 * 30, label: 'Last 30 days' },
] as const

/**
 * An hour is the acceptance criterion itself -- "what a specific error said
 * an hour ago" -- so it is the window a customer lands on rather than one
 * they have to choose.
 */
export const DEFAULT_SEARCH_WINDOW_MINUTES = 60

export interface SearchState {
  windowMinutes: number
  floor: SearchLevel
  text: string
}

export interface SearchRequest {
  from: string
  to: string
  level_floor: string
  q?: string
  deployment_id: string
}

/**
 * Turns a preset into the absolute span the endpoint requires.
 *
 * Absolute, not an offset from "now": computed once, here, at the moment the
 * search runs, so the request that goes out carries a fixed span rather than
 * one that would mean something different a second later.
 */
export function resolveWindow(minutes: number, now: Date): { from: string; to: string } {
  const to = now.getTime()
  const from = to - minutes * 60_000

  return { from: new Date(from).toISOString(), to: new Date(to).toISOString() }
}

/** The request the generated client sends, from what the three controls hold. */
export function buildSearchRequest(
  state: SearchState,
  deploymentId: string,
  now: Date,
): SearchRequest {
  const { from, to } = resolveWindow(state.windowMinutes, now)
  const text = state.text.trim()

  return {
    from,
    to,
    level_floor: state.floor,
    deployment_id: deploymentId,
    ...(text !== '' ? { q: text } : {}),
  }
}

export type ResultLevel = SearchLevel | 'unknown'

/**
 * What the index calls a hit's level, read defensively.
 *
 * The wire type is a plain string, not the closed set this screen knows
 * about. A value this screen does not recognise is treated the same as
 * `unknown` -- read as "we can't say how severe this was", never dropped and
 * never mistaken for a level it did not claim to be.
 */
export function asResultLevel(raw: string): ResultLevel {
  return (SEARCH_LEVELS as string[]).includes(raw) ? (raw as SearchLevel) : 'unknown'
}

/**
 * A hit's stamp, read in full.
 *
 * Unlike the live tail's `readStamp`, the date is kept: a search can span
 * days, and a time with no date is ambiguous the moment a result is not from
 * today. Rendered in UTC so two people reading the same result read the same
 * stamp, whatever their own clocks say.
 */
export function formatTimestamp(at: string): string {
  const moment = new Date(at)
  if (Number.isNaN(moment.getTime())) return at

  return `${moment.toISOString().slice(0, 19).replace('T', ' ')}Z`
}

/** What the screen says about how many lines answered, and how many are shown. */
export function summarize(totalHits: number, shown: number): string {
  if (totalHits === 0) return 'No matching lines'
  if (shown >= totalHits) return totalHits === 1 ? '1 line' : `${totalHits} lines`

  return `${shown} of ${totalHits} lines`
}

/**
 * What the screen says when the endpoint refuses a search.
 *
 * A 409 -- no search index configured on this installation -- is told apart
 * from an empty result on purpose: an empty result reads as "nothing
 * happened here", which would be a lie about a search that never ran.
 */
export function whySearchFailed(status: number): string {
  if (status === 409) {
    return 'Log search is not set up on this installation. Ask your platform operator to enable it.'
  }
  if (status === 403) return 'You may not search this deployment’s logs.'
  if (status === 404) return 'This deployment no longer exists.'
  if (status === 400) return 'That search is not valid. Narrow the time range and try again.'

  return `The control plane refused the search (HTTP ${status}).`
}

export interface FacetBucket {
  value: string
  count: number
}

/**
 * A facet's buckets, ordered for display.
 *
 * By count first, since that is what the facet panel is for -- and by value
 * once counts tie, so a re-render never reshuffles two buckets that are
 * otherwise equal.
 */
export function orderFacet(buckets: FacetBucket[]): FacetBucket[] {
  return [...buckets].sort((a, b) => b.count - a.count || a.value.localeCompare(b.value))
}

export interface FacetShare extends FacetBucket {
  /** This value's share of the facet's own total, 0 to 100 -- not of all hits. */
  percent: number
}

/**
 * `orderFacet`, decorated with each bucket's share of the facet.
 *
 * The share is of the facet's own total (e.g. every distinct source seen),
 * not of `total_hits` -- a facet only ever covers the field it names, so a
 * percentage against anything wider would not add up to 100.
 */
export function facetShares(buckets: FacetBucket[]): FacetShare[] {
  const ordered = orderFacet(buckets)
  const total = ordered.reduce((sum, bucket) => sum + bucket.count, 0)

  return ordered.map((bucket) => ({
    ...bucket,
    percent: total === 0 ? 0 : (bucket.count / total) * 100,
  }))
}

export interface LevelBarSegment {
  level: ResultLevel
  count: number
  /** Share of the segment within the bar, 0 to 100. Zero once nothing matched. */
  percent: number
}

/**
 * The level facet, laid out as a single bar rather than a set of numbers.
 *
 * Ordered least severe first, matching `SEARCH_LEVELS`, with `unknown` last
 * -- it is not a severity, so it does not belong among ones that are ranked.
 */
export function levelBreakdown(buckets: FacetBucket[]): LevelBarSegment[] {
  const byValue = new Map(buckets.map((bucket) => [bucket.value, bucket.count]))
  const order: ResultLevel[] = [...SEARCH_LEVELS, 'unknown']
  const total = buckets.reduce((sum, bucket) => sum + bucket.count, 0)

  return order
    .map((level) => ({ level, count: byValue.get(level) ?? 0 }))
    .filter((segment) => segment.count > 0)
    .map((segment) => ({
      ...segment,
      percent: total === 0 ? 0 : (segment.count / total) * 100,
    }))
}

/** A uuid, shortened to what a reader can hold a run of them in view by. */
export function shortId(id: string): string {
  return id.length <= 8 ? id : `${id.slice(0, 8)}…`
}

/** A count, grouped by thousands so "6840" reads as "6,840". */
export function formatCount(count: number): string {
  return count.toLocaleString('en-US')
}

/**
 * What the screen says once a search has answered.
 *
 * The 200-hit cap is told apart from a complete answer here, the same way
 * `whySearchFailed` tells a refusal apart from an empty result: the
 * reference's "6,840 logs found" would otherwise be a lie once a search
 * matches more than the cap.
 */
export function describeResults(totalHits: number, shown: number, elapsedMs: number): string {
  const timing = `in ${Math.max(0, Math.round(elapsedMs))} ms`

  if (totalHits === 0) return `No logs found ${timing}`
  if (shown >= totalHits) {
    return `${formatCount(totalHits)} ${totalHits === 1 ? 'log' : 'logs'} found ${timing}`
  }

  return `Showing the first ${formatCount(shown)} of ${formatCount(totalHits)} logs ${timing} — narrow the search to see the rest`
}
