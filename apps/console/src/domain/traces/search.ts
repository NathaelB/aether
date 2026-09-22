/**
 * Asking what happened to a request, rather than what a service logged
 * about it.
 *
 * Mirrors `domain/logs/search.ts`: a time range and some filters turn into
 * one request against the organisation's own trace index. The window
 * mechanics (`resolveWindow`, the presets) are shared outright rather than
 * duplicated -- both search the same Quickwit installation under the same
 * 30-day retention, so a second copy of "how a preset becomes an absolute
 * span" would only be a second place for the two to drift apart.
 */
import { SEARCH_WINDOWS, resolveWindow } from '../logs/search'

export { SEARCH_WINDOWS, resolveWindow }

export const DEFAULT_TRACE_WINDOW_MINUTES = 60

export interface TraceSearchState {
  windowMinutes: number
  serviceName: string
  statusCode: string
  text: string
}

export interface TraceSearchRequest {
  from: string
  to: string
  service_name?: string
  status_code?: string
  q?: string
  deployment_id: string
}

export function buildTraceSearchRequest(
  state: TraceSearchState,
  deploymentId: string,
  now: Date,
): TraceSearchRequest {
  const { from, to } = resolveWindow(state.windowMinutes, now)
  const text = state.text.trim()

  return {
    from,
    to,
    deployment_id: deploymentId,
    ...(state.serviceName !== '' ? { service_name: state.serviceName } : {}),
    ...(state.statusCode !== '' ? { status_code: state.statusCode } : {}),
    ...(text !== '' ? { q: text } : {}),
  }
}

/** A duration, read the way a human reads one rather than as a raw count of nanoseconds. */
export function formatDuration(nanos: number): string {
  if (nanos < 1_000) return `${nanos} ns`
  if (nanos < 1_000_000) return `${(nanos / 1_000).toFixed(1)} µs`
  if (nanos < 1_000_000_000) return `${(nanos / 1_000_000).toFixed(1)} ms`

  return `${(nanos / 1_000_000_000).toFixed(2)} s`
}

/** What the screen says once a search has answered -- mirrors `logs/search.ts`'s `describeResults`. */
export function describeResults(totalHits: number, shown: number, elapsedMs: number): string {
  const timing = `in ${Math.max(0, Math.round(elapsedMs))} ms`

  if (totalHits === 0) return `No traces found ${timing}`
  if (shown >= totalHits) {
    return `${totalHits} ${totalHits === 1 ? 'span' : 'spans'} found ${timing}`
  }

  return `Showing the first ${shown} of ${totalHits} spans ${timing} — narrow the search to see the rest`
}

/** What the screen says when the endpoint refuses a search -- mirrors `logs/search.ts`'s `whySearchFailed`. */
export function whySearchFailed(status: number): string {
  if (status === 409) {
    return 'Trace search is not set up on this installation. Ask your platform operator to enable it.'
  }
  if (status === 403) return 'You may not search this deployment’s traces.'
  if (status === 404) return 'This deployment no longer exists.'
  if (status === 400) return 'That search is not valid. Narrow the time range and try again.'

  return `The control plane refused the search (HTTP ${status}).`
}

/** A trace id, shortened to what a reader can hold a run of them in view by. */
export function shortTraceId(id: string): string {
  return id.length <= 12 ? id : `${id.slice(0, 12)}…`
}
