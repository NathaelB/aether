import { describe, expect, it } from 'vitest'
import type { CreateDeploymentForm } from './create-deployment-request'
import { toCreateDeploymentRequest } from './create-deployment-request'

function form(overrides: Partial<CreateDeploymentForm> = {}): CreateDeploymentForm {
  return {
    name: 'acme api',
    kind: 'ferriskey',
    version: '0.5.0',
    environment: 'production',
    offer: 'standard',
    ...overrides,
  }
}

describe('what the form sends', () => {
  /**
   * The six infrastructure decisions the request used to carry are gone. The
   * platform derives the namespace and the sizing from the offer and decides
   * where the thing runs, and refuses a request that still names any of them
   * rather than half-honouring it.
   */
  it('carries nothing the platform decides for itself', () => {
    expect(toCreateDeploymentRequest(form())).toEqual({
      name: 'acme api',
      kind: 'ferriskey',
      version: '0.5.0',
      environment: 'production',
      offer: 'standard',
    })
  })

  it('carries the offer the customer chose, whichever it is', () => {
    expect(toCreateDeploymentRequest(form({ offer: 'private' })).offer).toBe('private')
  })

  /**
   * A region is the fleet seen from outside: choosing one is choosing which
   * cluster serves you, which is an operator's decision. The API refuses a
   * request that names one, so sending it would turn every create into a 400.
   */
  it('names no region, because where a deployment runs is not the customer’s call', () => {
    expect(toCreateDeploymentRequest(form())).not.toHaveProperty('region')
  })
})
