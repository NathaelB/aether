import type { Schemas } from '@/api/api.client'

export type DeploymentStatus = Schemas.DeploymentStatus
export type DeploymentKind = Schemas.DeploymentKind

export type Deployment = Schemas.Deployment

/** Placement intent, not what a data plane is. */

export type Environment = 'production' | 'staging' | 'development'


export const KIND_LABELS: Record<DeploymentKind, string> = {
  ferriskey: 'FerrisKey',
  keycloak: 'Keycloak',
}

