import type { Schemas } from '@/api/api.client'
import type { DeploymentResources } from './resources'

export type DeploymentStatus = Schemas.DeploymentStatus
export type DeploymentKind = Schemas.DeploymentKind

export type Deployment = Schemas.Deployment

/** Placement intent, not what a data plane is. */
export type DeploymentMode = 'shared' | 'dedicated'

export type Environment = 'production' | 'staging' | 'development'
export type DeploymentSize = 'small' | 'medium' | 'large'

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
