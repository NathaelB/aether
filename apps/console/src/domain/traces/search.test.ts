import { describe, expect, it } from 'vitest'
import {
  buildTraceSearchRequest,
  describeResults,
  formatDuration,
  shortTraceId,
  whySearchFailed,
} from './search'

describe('buildTraceSearchRequest', () => {
  const now = new Date('2026-09-20T12:00:00.000Z')

  it('carries the deployment and the resolved window', () => {
    const request = buildTraceSearchRequest(
      { windowMinutes: 60, serviceName: '', statusCode: '', text: '' },
      'deployment-1',
      now,
    )

    expect(request).toEqual({
      from: '2026-09-20T11:00:00.000Z',
      to: '2026-09-20T12:00:00.000Z',
      deployment_id: 'deployment-1',
    })
  })

  it('leaves out filters that are unset, and includes those that are', () => {
    const request = buildTraceSearchRequest(
      { windowMinutes: 60, serviceName: 'ferriskey-api', statusCode: 'error', text: '  realm  ' },
      'deployment-1',
      now,
    )

    expect(request.service_name).toBe('ferriskey-api')
    expect(request.status_code).toBe('error')
    expect(request.q).toBe('realm')
  })
})

describe('formatDuration', () => {
  it('reads a sub-microsecond duration in nanoseconds', () => {
    expect(formatDuration(500)).toBe('500 ns')
  })

  it('reads a sub-millisecond duration in microseconds', () => {
    expect(formatDuration(1_500)).toBe('1.5 µs')
  })

  it('reads a sub-second duration in milliseconds', () => {
    expect(formatDuration(1_500_000)).toBe('1.5 ms')
  })

  it('reads a duration of a second or more in seconds', () => {
    expect(formatDuration(2_500_000_000)).toBe('2.50 s')
  })
})

describe('describeResults', () => {
  it('says plainly when nothing matched', () => {
    expect(describeResults(0, 0, 12)).toBe('No traces found in 12 ms')
  })

  it('states the total once every match is shown', () => {
    expect(describeResults(1, 1, 5)).toBe('1 span found in 5 ms')
    expect(describeResults(42, 42, 8)).toBe('42 spans found in 8 ms')
  })

  it('tells a capped result apart from a complete one', () => {
    expect(describeResults(300, 200, 40)).toBe(
      'Showing the first 200 of 300 spans in 40 ms — narrow the search to see the rest',
    )
  })
})

describe('whySearchFailed', () => {
  it('names the installation gap plainly', () => {
    expect(whySearchFailed(409)).toMatch(/not set up/)
  })

  it('falls back to the status code for anything unnamed', () => {
    expect(whySearchFailed(500)).toContain('500')
  })
})

describe('shortTraceId', () => {
  it('leaves a short id alone', () => {
    expect(shortTraceId('abc123')).toBe('abc123')
  })

  it('shortens a full 32-hex trace id', () => {
    expect(shortTraceId('11111111111111111111111111111111')).toBe('111111111111…')
  })
})
