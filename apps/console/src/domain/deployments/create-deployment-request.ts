import type { Schemas } from '@/api/api.client'
import {
  DEPLOYMENT_PLANS,
  type DeploymentMode,
  type DeploymentPlan,
  type DeploymentType,
  type Environment,
} from './types/deployment'

/**
 * What the create form collects, before it becomes an API request.
 *
 * Kept separate from `Schemas.CreateDeploymentRequest` on purpose: the form
 * speaks in product terms (a plan, an environment, an identity provider) and
 * the API speaks in placement terms (millicores, a namespace, a mode). The
 * translation between the two is the interesting part, so it lives in a pure
 * function that can be tested without rendering anything.
 */
export interface CreateDeploymentForm {
  name: string
  type: DeploymentType
  environment: Environment
  region: string
  mode: DeploymentMode
  plan: DeploymentPlan
}

/**
 * `authentik` is offered by the form and not implemented by the control plane,
 * whose `DeploymentKind` is `ferriskey | keycloak`. Mapping it onto Keycloak is
 * what the form already did; it is recorded here rather than inline so that the
 * lie is visible in one place, and so removing it is a one-line change.
 */
const KIND_BY_TYPE: Record<DeploymentType, Schemas.DeploymentKind> = {
  keycloak: 'keycloak',
  ferriskey: 'ferriskey',
  authentik: 'keycloak',
}

/**
 * Kubernetes namespaces are DNS-1123 labels: lowercase alphanumerics and
 * hyphens, starting and ending with an alphanumeric, at most 63 characters.
 *
 * The API does not validate this today, so a name with an apostrophe in it
 * reaches the operator and fails there — a long way from the person who typed
 * it. Doing it here does not make the API's silence safe, it just means the
 * console stops being the thing that sends bad names.
 */
export function toNamespace(environment: Environment, name: string): string {
  const slug = `${environment}-${name}`
    .toLowerCase()
    .normalize('NFKD')
    // Strip combining marks, so "développement" becomes "developpement"
    // rather than losing the letter entirely.
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/(^-+|-+$)/g, '')

  return slug.slice(0, 63).replace(/-+$/, '')
}

/**
 * Builds the request body from the form.
 *
 * Every field the form collects that the API can accept is sent. That is worth
 * stating because the previous version sent five of them and dropped the rest:
 * a user picked a region, a plan and a capacity, and the request carried none
 * of it, so every deployment landed on whatever the control plane's default
 * region was, at the default size, shared.
 */
export function toCreateDeploymentRequest(
  form: CreateDeploymentForm,
): Schemas.CreateDeploymentRequest {
  const { resources } = DEPLOYMENT_PLANS[form.plan]

  return {
    name: form.name,
    kind: KIND_BY_TYPE[form.type],
    version: 'latest',
    namespace: toNamespace(form.environment, form.name),
    region: form.region,
    mode: form.mode,
    // The API takes these three together or not at all: it rejects a request
    // that sets CPU without storage rather than completing it from a default,
    // on the grounds that a half-specified size is more likely a mistake than
    // an intent. A plan always carries all three, so the pairing holds here by
    // construction.
    cpu_millis: resources.cpuMillis,
    memory_mib: resources.memoryMib,
    storage_gib: resources.storageGib,
  }
}
