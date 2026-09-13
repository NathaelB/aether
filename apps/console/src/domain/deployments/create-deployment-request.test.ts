import { describe, expect, it } from 'vitest'
import type { CreateDeploymentForm } from './create-deployment-request'
import { toCreateDeploymentRequest, toOffer } from './create-deployment-request'

function form(overrides: Partial<CreateDeploymentForm> = {}): CreateDeploymentForm {
  return {
    name: 'acme api',
    kind: 'ferriskey',
    version: '0.5.0',
    environment: 'production',
    region: 'fr-par',
    mode: 'shared',
    size: 'medium',
    ...overrides,
  }
}

describe('what the form sends', () => {
  /**
   * The five infrastructure decisions the request used to carry are gone. The
   * platform derives the namespace and the sizing, and it refuses a request
   * that still names them rather than half-honouring it.
   */
  it('carries nothing the platform decides for itself', () => {
    const request = toCreateDeploymentRequest(form())

    expect(request).toEqual({
      name: 'acme api',
      kind: 'ferriskey',
      version: '0.5.0',
      environment: 'production',
      region: 'fr-par',
      offer: 'standard',
    })
  })

  it('still carries the region, which is the customer’s to choose', () => {
    expect(toCreateDeploymentRequest(form({ region: 'nl-ams' })).region).toBe('nl-ams')
  })
})

describe('mapping the old form onto an offer', () => {
  /**
   * A stopgap while the form still asks for a mode and a size. It goes away
   * with the form, in the version that offers a list of offers instead.
   */
  it('reads a dedicated request as the offer that gives a cluster of its own', () => {
    expect(toOffer('dedicated', 'small')).toBe('private')
    expect(toOffer('dedicated', 'large')).toBe('private')
  })

  it('maps each shared size onto the offer of that size', () => {
    expect(toOffer('shared', 'small')).toBe('sandbox')
    expect(toOffer('shared', 'medium')).toBe('standard')
    expect(toOffer('shared', 'large')).toBe('scale')
  })
})
