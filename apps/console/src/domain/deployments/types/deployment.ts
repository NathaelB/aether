import type { Schemas } from '@/api/api.client'
import type { DeploymentResources } from './resources'

export type DeploymentStatus = Schemas.DeploymentStatus
export type DeploymentKind = Schemas.DeploymentKind

export type Deployment = Schemas.Deployment

/** Placement intent, not what a data plane is. */
export type DeploymentMode = 'shared' | 'dedicated'

export type Environment = 'production' | 'staging' | 'development'
export type DeploymentSize = 'small' | 'medium' | 'large'

/**
 * The environment a deployment belongs to, read back out of its namespace.
 *
 * Namespaces are built as `{environment}-{name}`, so the environment is not
 * stored anywhere else. A namespace that does not follow the shape is shown
 * whole rather than cut at a separator that is not there.
 */
export function environmentOf(namespace: string): string {
  const [environment, ...rest] = namespace.split('-')
  if (rest.length === 0 || environment.length === 0) return namespace

  return environment.charAt(0).toUpperCase() + environment.slice(1)
}

export const KIND_LABELS: Record<DeploymentKind, string> = {
  ferriskey: 'FerrisKey',
  keycloak: 'Keycloak',
}

export const DEPLOYMENT_SIZES: Record<
  DeploymentSize,
  { label: string; description: string; resources: DeploymentResources }
> = {
  small: {
    label: 'Small',
    description: 'Development and small teams',
    resources: { cpuMillis: 500, memoryMib: 1024, storageGib: 5 },
  },
  medium: {
    label: 'Medium',
    description: 'Production workloads',
    resources: { cpuMillis: 2000, memoryMib: 4096, storageGib: 20 },
  },
  large: {
    label: 'Large',
    description: 'High traffic',
    resources: { cpuMillis: 4000, memoryMib: 8192, storageGib: 50 },
  },
}
