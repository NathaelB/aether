/**
 * The volume-over-time chart the search screen draws a spike on, and the
 * deployment's own actions placed on the same axis (V5, #298).
 *
 * The buckets come from the search response itself (`LogSearchResult.buckets`,
 * a `date_histogram` run against the whole matching set on the control
 * plane) rather than from `hits`, which `MAX_SEARCH_HITS` can cap well below
 * what actually matched -- bucketing the capped hits here would draw a
 * spike that is an artefact of the cap, not of the logs.
 */

import type { Schemas } from '@/api/api.client'

/** One bar of the histogram, in the axis's own unit: epoch milliseconds. */
export interface HistogramBar {
  at: number
  count: number
}

/** The buckets a search answered with, read into the chart's own shape, oldest first. */
export function toBars(buckets: Schemas.LogSearchBucket[]): HistogramBar[] {
  return buckets
    .map((bucket) => ({ at: Date.parse(bucket.start), count: bucket.count }))
    .filter((bar) => !Number.isNaN(bar.at))
    .sort((a, b) => a.at - b.at)
}

/** The tallest bar -- never zero, so an empty window still has a scale to plot bars against. */
export function maxCount(bars: HistogramBar[]): number {
  return Math.max(1, ...bars.map((bar) => bar.count))
}

/** One action, placed on the same axis as the bars. */
export interface ActionMarker {
  at: number
  label: string
}

/**
 * A namespaced action type (`"deployment.upgrade"`), read as the word a
 * customer recognises: its last segment, spaced out. Deliberately not a
 * fixed lookup table naming only upgrade/restore/cutover/drill -- the
 * actions endpoint returns whatever this deployment did, and a type this
 * screen does not special-case still deserves an honest label rather than
 * being dropped or mislabelled.
 */
export function actionLabel(actionType: string): string {
  const last = actionType.split('.').pop() || actionType
  return last.replace(/_/g, ' ')
}

/** The actions that landed inside `[from, to)`, oldest first -- the same window the bars cover. */
export function actionMarkers(
  actions: Schemas.Action[],
  from: number,
  to: number,
): ActionMarker[] {
  return actions
    .map((action) => ({
      at: Date.parse(action.metadata.created_at),
      label: actionLabel(action.action_type),
    }))
    .filter((marker) => !Number.isNaN(marker.at) && marker.at >= from && marker.at < to)
    .sort((a, b) => a.at - b.at)
}

/** Where an instant falls between `from` and `to`, as a fraction clamped to `[0, 1]`. */
export function xFraction(at: number, from: number, to: number): number {
  if (to <= from) return 0
  return Math.min(1, Math.max(0, (at - from) / (to - from)))
}
