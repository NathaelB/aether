import { describe, expect, it } from 'vitest'
import type { Schemas } from '@/api/api.client'
import { layoutWaterfall } from './waterfall'

function span(overrides: Partial<Schemas.SpanHit>): Schemas.SpanHit {
  return {
    trace_id: 'trace-1',
    span_id: 'span-1',
    parent_span_id: '',
    deployment_id: 'dep-1',
    service_name: 'ferriskey-api',
    name: 'operation',
    kind: 'server',
    start_timestamp: '2026-09-01T00:00:00.000Z',
    duration_nanos: 1_000_000,
    status_code: 'ok',
    status_message: '',
    ...overrides,
  }
}

describe('layoutWaterfall', () => {
  it('is empty for no spans', () => {
    expect(layoutWaterfall([])).toEqual([])
  })

  it('a lone root span spans the whole width at depth zero', () => {
    const [row] = layoutWaterfall([span({})])

    expect(row.depth).toBe(0)
    expect(row.offsetPercent).toBe(0)
    expect(row.widthPercent).toBe(100)
  })

  it('a child is offset from its parent and one level deeper', () => {
    const root = span({
      span_id: 'root',
      start_timestamp: '2026-09-01T00:00:00.000Z',
      duration_nanos: 1_000_000_000,
    })
    const child = span({
      span_id: 'child',
      parent_span_id: 'root',
      start_timestamp: '2026-09-01T00:00:00.500Z',
      duration_nanos: 200_000_000,
    })

    const rows = layoutWaterfall([root, child])
    const childRow = rows.find((row) => row.span.span_id === 'child')!

    expect(childRow.depth).toBe(1)
    expect(childRow.offsetPercent).toBeCloseTo(50, 0)
    expect(childRow.widthPercent).toBeCloseTo(20, 0)
  })

  it('a grandchild is two levels deep', () => {
    const root = span({ span_id: 'root' })
    const mid = span({ span_id: 'mid', parent_span_id: 'root' })
    const leaf = span({ span_id: 'leaf', parent_span_id: 'mid' })

    const rows = layoutWaterfall([root, mid, leaf])

    expect(rows.find((row) => row.span.span_id === 'leaf')!.depth).toBe(2)
  })

  it('a parent id nothing in the batch names is read as a root, not an error', () => {
    const orphan = span({ span_id: 'orphan', parent_span_id: 'missing' })

    expect(layoutWaterfall([orphan])[0].depth).toBe(0)
  })

  it('never produces a zero-width sliver for an instant span', () => {
    const instant = span({ duration_nanos: 0 })

    expect(layoutWaterfall([instant])[0].widthPercent).toBeGreaterThan(0)
  })

  it('does not infinite-loop on a cyclical parent chain', () => {
    const a = span({ span_id: 'a', parent_span_id: 'b' })
    const b = span({ span_id: 'b', parent_span_id: 'a' })

    expect(() => layoutWaterfall([a, b])).not.toThrow()
  })
})
