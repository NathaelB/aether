import type { Schemas } from '@/api/api.client'
import {
  type DeploymentKind,
  type DeploymentMode,
  type DeploymentSize,
  type Environment,
} from './types/deployment'

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
  region: string
  mode: DeploymentMode
  size: DeploymentSize
}

/**
 * Which offer this form amounts to.
 *
 * A stopgap. The form still asks for a mode and a size, which is exactly what
 * the platform stopped accepting -- so the two are mapped onto the offer that
 * matches them. The form is replaced by a list of offers in its own version,
 * and this function goes with it.
 */
export function toOffer(mode: DeploymentMode, size: DeploymentSize): Offer {
  if (mode === 'dedicated') return 'private'

  return size === 'small' ? 'sandbox' : size === 'large' ? 'scale' : 'standard'
}

export type Offer = 'sandbox' | 'standard' | 'scale' | 'private'

export function toCreateDeploymentRequest(
  form: CreateDeploymentForm,
): Schemas.CreateDeploymentRequest {
  return {
    name: form.name,
    kind: form.kind,
    version: form.version,
    environment: form.environment,
    region: form.region,
    offer: toOffer(form.mode, form.size),
  }
}
