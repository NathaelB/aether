import type { Schemas } from '@/api/api.client'

/**
 * Capacity in the units somebody sizing a machine thinks in.
 *
 * The API takes millicores and mebibytes, which is right for arithmetic and
 * wrong for a form: nobody reading a VPS invoice converts 4 vCPU to 4000
 * before typing it.
 */
export interface CapacityForm {
  vcpu: string
  memoryGib: string
  storageGib: string
  /**
   * The optional second bound (#270). Blank means exactly what it always
   * meant: no count bound, placement decided by resources alone.
   */
  maxDeployments: string
}

export interface RegistrationForm {
  region: string
  mode: Schemas.DataPlaneMode
  /** Required for `dedicated`, meaningless for `shared`. */
  organisationId: string | null
  capacity: CapacityForm
}

function positive(value: string): number | null {
  const parsed = Number(value.trim())

  return Number.isFinite(parsed) && parsed > 0 ? parsed : null
}

/**
 * A blank count bound is `undefined` here -- an operator who left it empty
 * meant "no bound", not zero. Anything else that is not a positive whole
 * number is `false`, distinct from `undefined` so a caller can tell a typo
 * apart from an intentionally empty field.
 */
function optionalDeploymentCount(value: string): number | undefined | false {
  const trimmed = value.trim()
  if (trimmed === '') return undefined

  const parsed = Number(trimmed)
  return Number.isInteger(parsed) && parsed > 0 ? parsed : false
}

/**
 * The capacity the API takes, or `null` when the form does not describe one.
 *
 * Rounded down: half a core is a number the platform can hold but not a
 * number anybody meant, and rounding up would declare capacity the machine
 * does not have.
 */
export function toCapacity(form: CapacityForm): Schemas.Capacity | null {
  const vcpu = positive(form.vcpu)
  const memory = positive(form.memoryGib)
  const storage = positive(form.storageGib)
  const maxDeployments = optionalDeploymentCount(form.maxDeployments)

  if (vcpu === null || memory === null || storage === null || maxDeployments === false) {
    return null
  }

  return {
    cpu_millis: Math.floor(vcpu * 1000),
    memory_mib: Math.floor(memory * 1024),
    storage_gib: Math.floor(storage),
    ...(maxDeployments !== undefined ? { max_deployments: maxDeployments } : {}),
  }
}

/**
 * What stops this form being sent, said the way it would be said to a person.
 *
 * `null` means nothing does. Checked here rather than left to the API because
 * the API's refusal arrives after the operator has left the form, and the
 * whole of this screen is the moment before that.
 */
export function whatIsMissing(form: RegistrationForm): string | null {
  if (form.region.trim() === '') return 'Name the region this cluster serves.'

  if (optionalDeploymentCount(form.capacity.maxDeployments) === false) {
    return 'The deployment limit must be a whole number greater than zero, or left blank.'
  }

  if (toCapacity(form.capacity) === null) {
    return 'Capacity must be more than nothing in all three: CPU, memory and storage.'
  }

  if (form.mode === 'dedicated' && !form.organisationId) {
    return 'A dedicated data plane is reserved to one organisation. Say which.'
  }

  return null
}

export function toCreateRequest(form: RegistrationForm): Schemas.CreateDataPlaneRequest | null {
  const capacity = toCapacity(form.capacity)
  if (capacity === null || whatIsMissing(form) !== null) return null

  return {
    region: form.region.trim(),
    mode: form.mode,
    capacity,
    // Rejected by the API for a shared plane rather than ignored, so it must
    // not travel at all.
    ...(form.mode === 'dedicated' ? { organisation_id: form.organisationId } : {}),
  }
}

export interface InstallDetails {
  dataplaneId: string
  /** Reachable from inside the cluster, which is rarely the browser's URL. */
  controlPlaneUrl: string
  issuerUrl: string
  clientId: string
  clientSecret: string
  namespace: string
}

/** Where a data plane's components live unless somebody says otherwise. */
export const DEFAULT_NAMESPACE = 'aether-system'

/**
 * The command that turns a registration into a running data plane.
 *
 * Run from a checkout: the chart is not published anywhere yet, so the path
 * is a path. Everything else is filled in, because the gap between a
 * credential shown once and a cluster that works should be one paste.
 */
export function helmCommand(details: InstallDetails): string {
  return [
    'helm upgrade --install aether-dataplane charts/aether-dataplane',
    `  --namespace ${details.namespace} --create-namespace`,
    `  --set dataplane.id=${details.dataplaneId}`,
    `  --set controlPlane.url=${details.controlPlaneUrl}`,
    `  --set controlPlane.auth.issuer=${details.issuerUrl}`,
    `  --set controlPlane.auth.clientId=${details.clientId}`,
    `  --set controlPlane.auth.clientSecret=${details.clientSecret}`,
  ].join(' \\\n')
}
