import type { Schemas } from '@/api/api.client'
import {
  DEPLOYMENT_SIZES,
  type DeploymentKind,
  type DeploymentMode,
  type DeploymentSize,
  type Environment,
} from './types/deployment'

export interface CreateDeploymentForm {
  name: string
  kind: DeploymentKind
  environment: Environment
  region: string
  mode: DeploymentMode
  size: DeploymentSize
}

/** DNS-1123 label: lowercase alphanumerics and hyphens, at most 63 characters. */
export function toNamespace(environment: Environment, name: string): string {
  const slug = `${environment}-${name}`
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/(^-+|-+$)/g, '')

  return slug.slice(0, 63).replace(/-+$/, '')
}

export function toCreateDeploymentRequest(
  form: CreateDeploymentForm,
): Schemas.CreateDeploymentRequest {
  const { resources } = DEPLOYMENT_SIZES[form.size]

  return {
    name: form.name,
    kind: form.kind,
    version: 'latest',
    namespace: toNamespace(form.environment, form.name),
    region: form.region,
    mode: form.mode,
    cpu_millis: resources.cpuMillis,
    memory_mib: resources.memoryMib,
    storage_gib: resources.storageGib,
  }
}
