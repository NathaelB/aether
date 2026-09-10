export interface DeploymentResources {
  cpuMillis: number
  memoryMib: number
  storageGib: number
}

export function formatCpu(cpuMillis: number): string {
  const cores = cpuMillis / 1000
  return `${Number.isInteger(cores) ? cores : cores.toFixed(1)} vCPU`
}

export function formatMemory(memoryMib: number): string {
  const gib = memoryMib / 1024
  return `${Number.isInteger(gib) ? gib : gib.toFixed(1)} GiB`
}

export function formatStorage(storageGib: number): string {
  return `${storageGib} GiB`
}
