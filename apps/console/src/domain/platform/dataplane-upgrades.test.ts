import { describe, expect, it } from 'vitest'
import type { Schemas } from '@/api/api.client'
import {
  statusLabel,
  targetVersion,
  toRequest,
  toRows,
  whatIsMissing,
  type UpgradeForm,
} from './dataplane-upgrades'

function form(over: Partial<UpgradeForm> = {}): UpgradeForm {
  return {
    targetVersion: '1.4.0',
    allDataplanes: false,
    dataplaneIds: ['dp-1'],
    components: ['Herald'],
    maxUnavailable: '1',
    ...over,
  }
}

function action(id: string, createdAt: string, version: unknown = '1.4.0'): Schemas.Action {
  return {
    id,
    action_type: 'dataplane.upgrade',
    dataplane_id: 'dp-1',
    metadata: { constraints: {}, created_at: createdAt, source: { User: { user_id: 'u' } } },
    payload: { data: { target_version: version } },
    status: 'Pending',
    target: { id: 'dp-1', kind: 'DataPlane' },
    version: 1,
  }
}

describe('the upgrade request', () => {
  it('sends rolling and the chosen ids', () => {
    expect(toRequest(form({ targetVersion: ' 1.4.0 ', maxUnavailable: '2' }))).toEqual({
      target_version: '1.4.0',
      dataplane_ids: ['dp-1'],
      components: ['Herald'],
      strategy: 'rolling',
      max_unavailable: 2,
    })
  })

  it('sends "all" for every data plane and for every component', () => {
    const request = toRequest(
      form({ allDataplanes: true, components: ['Herald', 'Genesis', 'Operator'] }),
    )

    expect(request.dataplane_ids).toBe('all')
    expect(request.components).toBe('all')
  })
})

describe('what is missing from the form', () => {
  it('accepts a complete form', () => {
    expect(whatIsMissing(form())).toBeUndefined()
  })

  it('asks for a version, a data plane, a component and a sane limit', () => {
    expect(whatIsMissing(form({ targetVersion: '  ' }))).toMatch(/version/)
    expect(whatIsMissing(form({ dataplaneIds: [] }))).toMatch(/data plane/)
    expect(whatIsMissing(form({ allDataplanes: true, dataplaneIds: [] }))).toBeUndefined()
    expect(whatIsMissing(form({ components: [] }))).toMatch(/component/)
    expect(whatIsMissing(form({ maxUnavailable: '0' }))).toMatch(/unavailable/)
    expect(whatIsMissing(form({ maxUnavailable: '1.5' }))).toMatch(/unavailable/)
    expect(whatIsMissing(form({ maxUnavailable: '' }))).toMatch(/unavailable/)
  })
})

describe('reading an action', () => {
  it('labels every status', () => {
    expect(statusLabel('Pending').label).toBe('Pending')
    expect(statusLabel({ Leased: { until: 'x' } }).label).toBe('Leased')
    expect(statusLabel({ Pulled: { agent_id: 'a', at: 'x' } }).label).toBe('Pulled')
    expect(statusLabel({ Published: { at: 'x' } })).toEqual({ label: 'Published', tone: 'success' })
    expect(statusLabel({ Failed: { at: 'x', reason: 'Timeout' } }).tone).toBe('danger')
  })

  it('reads the target version, or nothing when the payload has none', () => {
    expect(targetVersion({ data: { target_version: '2.0.0' } })).toBe('2.0.0')
    expect(targetVersion({ data: {} })).toBeNull()
    expect(targetVersion({ data: null })).toBeNull()
    expect(targetVersion({ data: 'x' })).toBeNull()
  })

  it('lists the newest action first across data planes', () => {
    const rows = toRows([
      { dataplane_id: 'dp-1', actions: [action('old', '2026-09-01T00:00:00Z')] },
      {
        dataplane_id: 'dp-2',
        actions: [action('new', '2026-09-02T00:00:00Z'), action('mid', '2026-09-01T12:00:00Z')],
      },
    ])

    expect(rows.map((row) => row.actionId)).toEqual(['new', 'mid', 'old'])
    expect(rows[0].dataplaneId).toBe('dp-2')
  })
})
