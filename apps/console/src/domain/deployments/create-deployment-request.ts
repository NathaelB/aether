import type { Schemas } from '@/api/api.client'
import { type DeploymentKind, type Environment } from './types/deployment'
import type { Offer } from './offers'

export interface CreateDeploymentForm {
  name: string
  kind: DeploymentKind
  /**
   * Exact, taken from the catalogue. It used to be the string `latest`, which
   * the platform refuses: a deployment records the version it runs, and a tag
   * that moves would make that record a lie the next time it moved.
   */
  version: string
  environment: Environment
  /** What they are buying. The size and the isolation come with it. */
  offer: Offer
}

export function toCreateDeploymentRequest(
  form: CreateDeploymentForm,
): Schemas.CreateDeploymentRequest {
  return {
    name: form.name,
    kind: form.kind,
    version: form.version,
    environment: form.environment,
    offer: form.offer,
  }
}
