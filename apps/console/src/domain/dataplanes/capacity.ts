import type { Schemas } from '@/api/api.client'

export interface DimensionUsage {
  used: number
  total: number
  /** 0–100, clamped. `0` when the dimension has no capacity at all. */
  percent: number
}

export interface CapacityUsage {
  cpuMillis: DimensionUsage
  memoryMib: DimensionUsage
  storageGib: DimensionUsage
}

function dimension(used: number, total: number): DimensionUsage {
  // A data plane cannot be created with zero capacity -- `Capacity::new`
  // rejects it -- but a response is data, not a promise, and dividing by it
  // would put `NaN%` on the page.
  const percent = total > 0 ? Math.min(100, Math.round((used / total) * 100)) : 0

  return { used, total, percent }
}

/**
 * How much of a data plane its deployments have reserved.
 *
 * Reserved, not consumed: these are the numbers placement subtracted when it
 * chose this plane (#51), not anything measured on the cluster. A deployment
 * sitting at 3% CPU still holds every millicore it asked for, because that is
 * what stopped another deployment being placed on top of it.
 *
 * Deleted deployments are excluded and pending ones are not. A pending
 * deployment has already been placed here -- its room is spoken for, and
 * showing it as free would misrepresent what the next placement can use.
 */
export function capacityUsage(
  capacity: Schemas.Capacity,
  deployments: Schemas.Deployment[],
): CapacityUsage {
  const live = deployments.filter((deployment) => !deployment.deleted_at)

  const sum = (pick: (resources: Schemas.DeploymentResources) => number) =>
    live.reduce((total, deployment) => total + pick(deployment.resources), 0)

  return {
    cpuMillis: dimension(sum((r) => r.cpu_millis), capacity.cpu_millis),
    memoryMib: dimension(sum((r) => r.memory_mib), capacity.memory_mib),
    storageGib: dimension(sum((r) => r.storage_gib), capacity.storage_gib),
  }
}

/**
 * Who a data plane belongs to.
 *
 * `DataPlaneAllocation` is a tagged union rather than a flag, so ownership
 * cannot be read off a boolean and the dedicated case always carries the
 * organisation it belongs to (#52).
 */
export function allocationOwner(allocation: Schemas.DataPlaneAllocation): string | null {
  return allocation === 'Shared' ? null : allocation.Dedicated.organisation_id
}

export function allocationLabel(allocation: Schemas.DataPlaneAllocation): 'Shared' | 'Dedicated' {
  return allocation === 'Shared' ? 'Shared' : 'Dedicated'
}
