export type MetricKey = 'requests' | 'token_events' | 'logins' | 'active_users'

export const METRIC_LABELS: Record<MetricKey, string> = {
  requests: 'Requests',
  token_events: 'Token events',
  logins: 'Logins',
  active_users: 'Active users',
}

/**
 * How several reported minutes collapse into one point on screen.
 *
 * Three of these are counts of things that happened, so they add up. Active
 * users is a level rather than a count: the same person active in three
 * minutes is one user, and adding would report three. The read side already
 * refuses to sum it, and a chart that summed anyway would disagree with the
 * number printed beside it.
 */
export type Aggregation = 'sum' | 'last'

export function aggregationFor(metric: MetricKey): Aggregation {
  return metric === 'active_users' ? 'last' : 'sum'
}

/** A point on the chart. `null` means nothing was reported, which is not zero. */
export interface Point {
  at: number
  value: number | null
}

export interface Bucket {
  bucket: string
  value: number
}

/**
 * Collapses reported minutes into `slots` evenly spaced points.
 *
 * A slot with nothing reported in it stays null all the way to the chart,
 * because a data plane that could not be reached and one that reported
 * nobody are different facts and a zero would blur them.
 */
export function toSeries(
  buckets: Bucket[],
  from: number,
  until: number,
  slots: number,
  aggregation: Aggregation,
): Point[] {
  const width = (until - from) / slots
  const collected: number[][] = Array.from({ length: slots }, () => [])

  for (const bucket of buckets) {
    const at = Date.parse(bucket.bucket)
    if (Number.isNaN(at) || at < from || at >= until) continue

    const slot = Math.min(Math.floor((at - from) / width), slots - 1)
    collected[slot].push(bucket.value)
  }

  return collected.map((values, slot) => ({
    at: from + slot * width,
    value: values.length === 0 ? null : reduce(values, aggregation),
  }))
}

function reduce(values: number[], aggregation: Aggregation): number {
  if (aggregation === 'last') return values[values.length - 1]
  return values.reduce((total, value) => total + value, 0)
}

/**
 * The runs of consecutive reported points, so the chart can draw one line per
 * run and leave the gaps empty. One path across a gap would draw a straight
 * line through hours nobody reported.
 */
export function segments(points: Point[]): Point[][] {
  const runs: Point[][] = []
  let current: Point[] = []

  for (const point of points) {
    if (point.value === null) {
      if (current.length > 0) runs.push(current)
      current = []
      continue
    }
    current.push(point)
  }

  if (current.length > 0) runs.push(current)
  return runs
}

/**
 * The values to label the vertical axis with.
 *
 * Taken from the data rather than from a rounded scale, so every label names
 * a value the line actually reaches. A label at 100 above a line that peaks
 * at 37 tells the reader something false about the shape they are looking at.
 */
export function axisTicks(points: Point[]): number[] {
  const values = points
    .map((point) => point.value)
    .filter((value): value is number => value !== null)

  if (values.length === 0) return []

  const low = Math.min(...values)
  const high = Math.max(...values)
  if (low === high) return [high]

  const middle = Math.round((low + high) / 2)
  return middle === low || middle === high ? [low, high] : [low, middle, high]
}

/**
 * How the second half of the period compares with the first, as a fraction.
 *
 * Null when either half reported nothing: a period with no data to compare
 * has no growth, and showing 0% would read as "flat" rather than "unknown".
 */
export function growth(points: Point[], aggregation: Aggregation): number | null {
  const half = Math.floor(points.length / 2)
  const before = total(points.slice(0, half), aggregation)
  const after = total(points.slice(half), aggregation)

  if (before === null || after === null || before === 0) return null

  return (after - before) / before
}

function total(points: Point[], aggregation: Aggregation): number | null {
  const values = points
    .map((point) => point.value)
    .filter((value): value is number => value !== null)

  if (values.length === 0) return null
  return reduce(values, aggregation)
}

export function formatCount(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}k`
  return String(Math.round(value))
}
