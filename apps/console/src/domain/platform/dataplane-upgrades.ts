import type { Schemas } from '@/api/api.client'

export const COMPONENTS = ['Herald', 'Genesis', 'Operator'] as const

export type UpgradeComponent = (typeof COMPONENTS)[number]

export interface UpgradeForm {
  targetVersion: string
  allDataplanes: boolean
  dataplaneIds: string[]
  components: UpgradeComponent[]
  maxUnavailable: string
}

export interface UpgradeRow {
  actionId: string
  dataplaneId: string
  targetVersion: string | null
  status: StatusLabel
  createdAt: string
}

export interface StatusLabel {
  label: string
  tone: 'neutral' | 'progress' | 'success' | 'danger'
}

export function toRequest(form: UpgradeForm): Schemas.UpgradeDataplanesRequest {
  const everyComponent = COMPONENTS.every((component) => form.components.includes(component))

  return {
    target_version: form.targetVersion.trim(),
    dataplane_ids: form.allDataplanes ? 'all' : form.dataplaneIds,
    components: everyComponent ? 'all' : form.components,
    strategy: 'rolling',
    max_unavailable: Number(form.maxUnavailable),
  }
}

export function whatIsMissing(form: UpgradeForm): string | undefined {
  if (form.targetVersion.trim() === '') return 'Say which version to upgrade to.'
  if (!form.allDataplanes && form.dataplaneIds.length === 0) return 'Pick at least one data plane.'
  if (form.components.length === 0) return 'Pick at least one component.'

  const maxUnavailable = Number(form.maxUnavailable)
  if (!Number.isInteger(maxUnavailable) || maxUnavailable < 1) {
    return 'At most unavailable must be a whole number, 1 or more.'
  }

  return undefined
}

export function statusLabel(status: Schemas.ActionStatus): StatusLabel {
  if (status === 'Pending') return { label: 'Pending', tone: 'neutral' }
  if ('Leased' in status) return { label: 'Leased', tone: 'progress' }
  if ('Pulled' in status) return { label: 'Pulled', tone: 'progress' }
  if ('Published' in status) return { label: 'Published', tone: 'success' }
  return { label: 'Failed', tone: 'danger' }
}

export function targetVersion(payload: Schemas.ActionPayload): string | null {
  const data = payload.data
  if (typeof data !== 'object' || data === null) return null

  const version = (data as Record<string, unknown>).target_version
  return typeof version === 'string' ? version : null
}

export function toRows(upgrades: Schemas.DataplaneUpgradeActions[]): UpgradeRow[] {
  return upgrades
    .flatMap(({ dataplane_id, actions }) =>
      actions.map((action) => ({
        actionId: action.id,
        dataplaneId: dataplane_id,
        targetVersion: targetVersion(action.payload),
        status: statusLabel(action.status),
        createdAt: action.metadata.created_at,
      })),
    )
    .sort((a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt))
}
