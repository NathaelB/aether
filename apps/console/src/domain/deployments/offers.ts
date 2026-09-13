import type { Schemas } from '@/api/api.client'

export type Offer = Schemas.Offer
export type OfferAvailability = Schemas.OfferAvailability

/**
 * What each offer is, in the words a customer uses.
 *
 * Only the copy lives here. What an offer gives, whether it shares a cluster
 * and which tier opens it all come from the platform -- restating any of that
 * in TypeScript would be a second table, and the one that drifts is always the
 * one nobody looks at.
 */
export const OFFER_COPY: Record<Offer, { label: string; description: string }> = {
  sandbox: {
    label: 'Sandbox',
    description: 'Enough to try things against. Shares a cluster with other organisations.',
  },
  standard: {
    label: 'Standard',
    description: 'The everyday one, for an identity provider people actually sign in to.',
  },
  scale: {
    label: 'Scale',
    description: 'Room to grow into, still on a shared cluster.',
  },
  private: {
    label: 'Private',
    description: 'A Kubernetes cluster of your own. Nothing else runs on it.',
  },
}

/** The tier as the platform names it, rendered as a reader expects it. */
function tierName(plan: string): string {
  return plan.charAt(0).toUpperCase() + plan.slice(1).toLowerCase()
}

/**
 * Why an offer cannot be chosen, said as the next step rather than as a
 * refusal.
 *
 * `null` when it can. Hiding a closed offer makes an upsell invisible; showing
 * it without saying what opens it makes the reader guess between four tiers.
 */
export function whyClosed(entry: OfferAvailability): string | null {
  if (entry.open) return null

  const tier = entry.opened_by ? tierName(entry.opened_by) : undefined

  return tier ? `Available from the ${tier} plan.` : 'Not available on your plan.'
}

/** What an offer gives, in one line. */
export function describeResources(entry: OfferAvailability): string {
  const { cpu_millis, memory_mib, storage_gib } = entry.resources

  return `${formatCpu(cpu_millis)} · ${formatMemory(memory_mib)} · ${storage_gib} Gi disk`
}

function formatCpu(millis: number): string {
  return millis >= 1000 ? `${millis / 1000} vCPU` : `${millis}m vCPU`
}

function formatMemory(mib: number): string {
  return mib >= 1024 ? `${mib / 1024} Gi RAM` : `${mib} Mi RAM`
}

/**
 * Which offer a form should start on.
 *
 * The first one the organisation may actually choose, so a customer whose tier
 * opens two of four does not open the page on a disabled card. `undefined`
 * when the tier opens nothing, which is a state the screen has to say out loud
 * rather than present as an empty form.
 */
export function firstOpen(catalogue: OfferAvailability[]): Offer | undefined {
  return catalogue.find((entry) => entry.open)?.offer
}

/**
 * What moving an existing deployment to another offer means.
 *
 * Said on the deployment's own screen, because the honest answer is not a
 * slider: an offer is chosen when a deployment is created, and changing it is
 * a new deployment and a cutover.
 */
export const HOW_TO_CHANGE_OFFER =
  'An offer is chosen when a deployment is created. Moving to another one means creating a new deployment and cutting the hostname over to it, so you can check it before anything depends on it.'
