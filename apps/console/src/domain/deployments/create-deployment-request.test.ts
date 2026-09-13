import { describe, expect, it } from 'vitest'
import type { CreateDeploymentForm } from './create-deployment-request'
import { toCreateDeploymentRequest } from './create-deployment-request'

function form(overrides: Partial<CreateDeploymentForm> = {}): CreateDeploymentForm {
  return {
    name: 'acme api',
    kind: 'ferriskey',
    version: '0.5.0',
    environment: 'production',
    region: 'fr-par',
    offer: 'standard',
    ...overrides,
  }
}

describe('what the form sends', () => {
  /**
   * The five infrastructure decisions the request used to carry are gone. The
   * platform derives the namespace and the sizing from the offer, and refuses
   * a request that still names them rather than half-honouring it.
   */
  it('carries nothing the platform decides for itself', () => {
    expect(toCreateDeploymentRequest(form())).toEqual({
      name: 'acme api',
      kind: 'ferriskey',
      version: '0.5.0',
      environment: 'production',
      region: 'fr-par',
      offer: 'standard',
    })
  })

  it('carries the offer the customer chose, whichever it is', () => {
    expect(toCreateDeploymentRequest(form({ offer: 'private' })).offer).toBe('private')
  })

  it('still carries the region, which is the customer’s to choose', () => {
    expect(toCreateDeploymentRequest(form({ region: 'nl-ams' })).region).toBe('nl-ams')
  })
})
