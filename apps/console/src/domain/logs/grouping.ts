/**
 * Grouping the same stored search by the fingerprint computed at ingest time
 * (V4, #297): a burst of one error reads as one signature with a count,
 * rather than a wall of identical lines.
 *
 * The request is exactly `SearchRequest` -- the grouping endpoint takes the
 * same time range, level floor, free text and deployment a search does, and
 * only differs in what it does with what matches.
 */

import type { Schemas } from '@/api/api.client'

/**
 * What "new" means here, spelled out once so every place that shows the
 * badge says the same, honest thing: absent from the equivalent span of time
 * immediately before this one, not "never seen before". A window reaching
 * past the index's own 30-day retention has no baseline to compare against,
 * so everything in it reads as new too.
 */
export const NEW_SIGNATURE_HINT =
  'Not seen in the equivalent span of time right before this window -- not necessarily new overall.'

export function summarizeSignatures(signatures: Schemas.LogSignature[]): string {
  if (signatures.length === 0) return 'No signatures'

  const newCount = signatures.filter((signature) => signature.is_new).length
  const base = signatures.length === 1 ? '1 signature' : `${signatures.length} signatures`

  return newCount === 0 ? base : `${base}, ${newCount} new`
}

/** What the screen says when the grouping endpoint refuses a request. */
export function whyGroupFailed(status: number): string {
  if (status === 409) {
    return 'Log grouping is not set up on this installation. Ask your platform operator to enable it.'
  }
  if (status === 403) return 'You may not search this deployment’s logs.'
  if (status === 404) return 'This deployment no longer exists.'
  if (status === 400) return 'That search is not valid. Narrow the time range and try again.'

  return `The control plane refused the grouping (HTTP ${status}).`
}
