import type { Schemas } from '@/api/api.client'

export type IamFeature = Schemas.IamFeature
export type IamFeatureAvailability = Schemas.IamFeatureAvailability

export const IAM_FEATURE_LABELS: Record<IamFeature, string> = {
  sso_connectors: 'SSO connectors',
  mfa: 'Multi-factor authentication',
  directory_federation: 'Directory federation',
  custom_domain: 'Custom domain',
  branding: 'Branding',
  analytics: 'Analytics',
  compliance: 'Compliance',
  delegated_admin: 'Delegated administration',
}

export function iamFeatureLabel(feature: IamFeature): string {
  return IAM_FEATURE_LABELS[feature]
}

/**
 * Not loaded yet and not listed both read as closed: a screen that guessed
 * open would show a feature the platform is about to refuse.
 */
export function isFeatureOpen(
  features: readonly IamFeatureAvailability[] | null | undefined,
  feature: IamFeature,
): boolean {
  return features?.find((entry) => entry.feature === feature)?.open === true
}
