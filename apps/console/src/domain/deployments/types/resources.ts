/**
 * The resource numbers a plan actually reserves.
 *
 * These are the units the API speaks: millicores, mebibytes, gibibytes. The
 * plan cards used to carry `'1 vCPU'` and `'2 GiB'` as free text, which was
 * fine while nothing was sent — the create form dropped the plan before the
 * request. Now that the numbers travel, a label and a reservation that can
 * disagree is a bug waiting to happen, so the label is derived from the number
 * rather than written beside it.
 */
export interface DeploymentResources {
  cpuMillis: number
  memoryMib: number
  storageGib: number
}

/** `500` -> `0.5 vCPU`, `2000` -> `2 vCPU`. */
export function formatCpu(cpuMillis: number): string {
  const cores = cpuMillis / 1000
  return `${Number.isInteger(cores) ? cores : cores.toFixed(1)} vCPU`
}

/** `512` -> `0.5 GiB`, `2048` -> `2 GiB`. */
export function formatMemory(memoryMib: number): string {
  const gib = memoryMib / 1024
  return `${Number.isInteger(gib) ? gib : gib.toFixed(1)} GiB`
}

export function formatStorage(storageGib: number): string {
  return `${storageGib} GiB`
}
