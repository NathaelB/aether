import type { Schemas } from '@/api/api.client'
import type { DeploymentResources } from './resources'

export type DeploymentStatus = Schemas.DeploymentStatus
export type DeploymentKind = Schemas.DeploymentKind

// Re-export the API Deployment type as the canonical Deployment type
export type Deployment = Schemas.Deployment

/**
 * Where the deployment runs: on infrastructure shared with other
 * organisations, or on a cluster of this organisation's own.
 *
 * This is placement *intent* -- the API's `mode` field. It is not the same as
 * what a data plane *is*, which the control plane calls an allocation and the
 * console never sets.
 */
export type DeploymentMode = 'shared' | 'dedicated';

export type DeploymentType = 'keycloak' | 'ferriskey' | 'authentik';
export type Environment = 'production' | 'staging' | 'development';
export type DeploymentPlan = 'freemium' | 'starter' | 'essential' | 'premium' | 'max';

export interface Project {
  id: string;
  name: string;
  organization: string;
}

export interface DeploymentFilters {
  search: string;
  status?: DeploymentStatus;
  kind?: DeploymentKind;
}

export const DEPLOYMENT_CAPACITIES = [100, 250, 500, 1000, 2500, 5000, 10000]

export const DEPLOYMENT_PLANS: Record<DeploymentPlan, {
    label: string;
    description: string;
    maxRealms: number;
    basePrice: number;
    /** What this plan reserves on a data plane. Sent with the request. */
    resources: DeploymentResources;
}> = {
  'freemium': {
      label: 'Freemium',
      resources: { cpuMillis: 500, memoryMib: 512, storageGib: 1 },
      description: 'For hobby projects',
      maxRealms: 1,
      basePrice: 0
  },
  'starter': {
      label: 'Starter',
      resources: { cpuMillis: 1000, memoryMib: 2048, storageGib: 5 },
      description: 'Entry-level for small teams',
      maxRealms: 100,
      basePrice: 20
  },
  'essential': {
      label: 'Essential',
      resources: { cpuMillis: 2000, memoryMib: 4096, storageGib: 10 },
      description: 'For growing businesses',
      maxRealms: 100,
      basePrice: 50
  },
  'premium': {
      label: 'Premium',
      resources: { cpuMillis: 4000, memoryMib: 8192, storageGib: 20 },
      description: 'High performance for scale',
      maxRealms: 100,
      basePrice: 100
  },
  'max': {
      label: 'Max',
      resources: { cpuMillis: 8000, memoryMib: 16384, storageGib: 50 },
      description: 'Mission critical workloads',
      maxRealms: 100,
      basePrice: 200
  },
}
