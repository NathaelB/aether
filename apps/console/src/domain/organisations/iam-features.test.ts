import { describe, expect, it } from 'vitest'
import { IAM_FEATURE_LABELS, iamFeatureLabel, isFeatureOpen } from './iam-features'
import type { IamFeature, IamFeatureAvailability } from './iam-features'

const open = (feature: IamFeature): IamFeatureAvailability => ({
  feature,
  open: true,
  opened_by: null,
})

describe('isFeatureOpen', () => {
  it('is open when the platform says so', () => {
    expect(isFeatureOpen([open('mfa')], 'mfa')).toBe(true)
  })

  it('is closed when the platform says so', () => {
    const closed: IamFeatureAvailability = { feature: 'mfa', open: false, opened_by: 'Business' }

    expect(isFeatureOpen([closed], 'mfa')).toBe(false)
  })

  it('is closed while the features are not loaded', () => {
    expect(isFeatureOpen(undefined, 'mfa')).toBe(false)
    expect(isFeatureOpen(null, 'mfa')).toBe(false)
  })

  it('is closed for a feature the answer does not list', () => {
    expect(isFeatureOpen([open('branding')], 'mfa')).toBe(false)
    expect(isFeatureOpen([], 'mfa')).toBe(false)
  })
})

describe('iamFeatureLabel', () => {
  it('names every feature', () => {
    for (const feature of Object.keys(IAM_FEATURE_LABELS) as IamFeature[]) {
      expect(iamFeatureLabel(feature).length).toBeGreaterThan(0)
    }
    expect(Object.keys(IAM_FEATURE_LABELS)).toHaveLength(8)
  })
})
