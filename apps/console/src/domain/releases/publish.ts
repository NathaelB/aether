import type { Schemas } from '@/api/api.client'
import { isVersion } from '@/domain/upgrades/version'

export interface PublishForm {
  version: string
  risk: Schemas.BreakingRisk
  notes: string
  /** Versions that must be passed through to reach this one, comma separated. */
  stepsThrough: string
  minimumOperatorVersion: string
}

export type PublishResult =
  | { request: Schemas.PublishReleaseRequest }
  | { errors: Partial<Record<keyof PublishForm, string>> }

export const EMPTY_PUBLISH_FORM: PublishForm = {
  version: '',
  risk: 'none',
  notes: '',
  stepsThrough: '',
  minimumOperatorVersion: '',
}

/**
 * Turns what the operator typed into what the catalogue accepts.
 *
 * Every version is checked here rather than left to the API, not because the
 * API would let a bad one through, but because a form that submits and comes
 * back with an error has already lost the operator's place in it.
 */
export function validatePublish(form: PublishForm): PublishResult {
  const errors: Partial<Record<keyof PublishForm, string>> = {}

  const version = form.version.trim()
  if (!version) {
    errors.version = 'A version is required.'
  } else if (!isVersion(version)) {
    errors.version = `'${version}' is not a version. Give an exact one, such as 26.0.1.`
  }

  const steps = splitList(form.stepsThrough)
  const badStep = steps.find((step) => !isVersion(step))
  if (badStep) {
    errors.stepsThrough = `'${badStep}' is not a version.`
  } else if (steps.some((step) => step === version)) {
    // A release is not a stepping stone to itself, and the platform drops it
    // silently, so saying so here is the only place it gets noticed.
    errors.stepsThrough = 'A version cannot be a step towards itself.'
  }

  const minimum = form.minimumOperatorVersion.trim()
  if (minimum && !isVersion(minimum)) {
    errors.minimumOperatorVersion = `'${minimum}' is not a version.`
  }

  if (Object.keys(errors).length > 0) return { errors }

  return {
    request: {
      version,
      risk: form.risk,
      ...(form.notes.trim() ? { notes: form.notes.trim() } : {}),
      ...(steps.length > 0 ? { steps_through: steps } : {}),
      ...(minimum ? { minimum_operator_version: minimum } : {}),
    },
  }
}

function splitList(input: string): string[] {
  return input
    .split(',')
    .map((part) => part.trim())
    .filter((part) => part.length > 0)
}
