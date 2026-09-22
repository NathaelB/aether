/**
 * Laying a trace's spans out as a waterfall: each span's horizontal position
 * and width relative to the trace's own span, and how deep it sits in the
 * parent/child tree.
 *
 * Deliberately simple, matching the plan's own "first cut" scope: no service
 * map, no flamegraph merging of concurrent siblings -- just enough geometry
 * to read which span started when, how long it took, and which span called
 * it.
 */
import type { Schemas } from '@/api/api.client'

export interface WaterfallRow {
  span: Schemas.SpanHit
  /** How many ancestors this span has within the same trace. Zero for a root. */
  depth: number
  /** Left edge, as a percentage of the trace's own total span. */
  offsetPercent: number
  /** Width, as a percentage of the trace's own total span -- never quite
   * zero, so an instant span is still a visible sliver rather than nothing. */
  widthPercent: number
}

export function layoutWaterfall(spans: Schemas.SpanHit[]): WaterfallRow[] {
  if (spans.length === 0) return []

  const starts = spans.map((span) => Date.parse(span.start_timestamp))
  const ends = spans.map((span, index) => starts[index] + span.duration_nanos / 1_000_000)
  const earliest = Math.min(...starts)
  const latest = Math.max(...ends)
  const total = Math.max(1, latest - earliest)

  const bySpanId = new Map(spans.map((span) => [span.span_id, span]))

  const depthOf = (span: Schemas.SpanHit, seen: Set<string>): number => {
    if (!span.parent_span_id || seen.has(span.span_id)) return 0
    const parent = bySpanId.get(span.parent_span_id)
    if (!parent) return 0

    seen.add(span.span_id)
    return 1 + depthOf(parent, seen)
  }

  return spans.map((span, index) => ({
    span,
    depth: depthOf(span, new Set()),
    offsetPercent: ((starts[index] - earliest) / total) * 100,
    widthPercent: Math.max(0.5, ((ends[index] - starts[index]) / total) * 100),
  }))
}
