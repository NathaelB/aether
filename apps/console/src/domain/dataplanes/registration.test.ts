import { describe, expect, it } from 'vitest'
import {
  DEFAULT_NAMESPACE,
  helmCommand,
  toCapacity,
  toCreateRequest,
  whatIsMissing,
  type RegistrationForm,
} from './registration'

function form(overrides: Partial<RegistrationForm> = {}): RegistrationForm {
  return {
    region: 'fr-par',
    mode: 'shared',
    organisationId: null,
    capacity: { vcpu: '4', memoryGib: '8', storageGib: '160' },
    ...overrides,
  }
}

describe('toCapacity', () => {
  /** Nobody reading a VPS invoice converts 4 vCPU to 4000 before typing it. */
  it('converts the units an operator thinks in', () => {
    expect(toCapacity({ vcpu: '4', memoryGib: '8', storageGib: '160' })).toEqual({
      cpu_millis: 4000,
      memory_mib: 8192,
      storage_gib: 160,
    })
  })

  /** Rounding up would declare capacity the machine does not have. */
  it('rounds down rather than up', () => {
    expect(toCapacity({ vcpu: '1.5', memoryGib: '3.7', storageGib: '20.9' })).toEqual({
      cpu_millis: 1500,
      memory_mib: 3788,
      storage_gib: 20,
    })
  })

  it('is nothing when a dimension is missing, zero or not a number', () => {
    expect(toCapacity({ vcpu: '', memoryGib: '8', storageGib: '160' })).toBeNull()
    expect(toCapacity({ vcpu: '4', memoryGib: '0', storageGib: '160' })).toBeNull()
    expect(toCapacity({ vcpu: '4', memoryGib: '8', storageGib: 'lots' })).toBeNull()
    expect(toCapacity({ vcpu: '-2', memoryGib: '8', storageGib: '160' })).toBeNull()
  })
})

describe('whatIsMissing', () => {
  it('is nothing for a form that describes a data plane', () => {
    expect(whatIsMissing(form())).toBeNull()
  })

  it('wants a region', () => {
    expect(whatIsMissing(form({ region: '   ' }))).toContain('region')
  })

  it('wants capacity in all three dimensions', () => {
    expect(
      whatIsMissing(form({ capacity: { vcpu: '4', memoryGib: '8', storageGib: '0' } })),
    ).toContain('Capacity')
  })

  /** A dedicated plane with no owner is one nothing can be placed on. */
  it('wants an organisation for a dedicated plane', () => {
    expect(whatIsMissing(form({ mode: 'dedicated' }))).toContain('organisation')
  })

  it('is satisfied once the dedicated plane has one', () => {
    expect(
      whatIsMissing(form({ mode: 'dedicated', organisationId: 'acme' })),
    ).toBeNull()
  })
})

describe('toCreateRequest', () => {
  it('sends the region trimmed and the capacity converted', () => {
    expect(toCreateRequest(form({ region: ' fr-par ' }))).toEqual({
      region: 'fr-par',
      mode: 'shared',
      capacity: { cpu_millis: 4000, memory_mib: 8192, storage_gib: 160 },
    })
  })

  /** The API refuses an owner on a shared plane rather than ignoring it, so
   * it must not travel at all. */
  it('carries no organisation for a shared plane, even if the form holds one', () => {
    expect(toCreateRequest(form({ organisationId: 'acme' }))).not.toHaveProperty(
      'organisation_id',
    )
  })

  it('carries it for a dedicated one', () => {
    expect(toCreateRequest(form({ mode: 'dedicated', organisationId: 'acme' }))).toMatchObject({
      organisation_id: 'acme',
    })
  })

  it('is nothing when the form is not ready', () => {
    expect(toCreateRequest(form({ region: '' }))).toBeNull()
  })
})

describe('helmCommand', () => {
  const details = {
    dataplaneId: '0192-abcd',
    controlPlaneUrl: 'https://api.aether.example',
    issuerUrl: 'https://id.aether.example/realms/aether',
    clientId: 'herald-0192-abcd',
    clientSecret: 'a-secret',
    namespace: DEFAULT_NAMESPACE,
  }

  it('carries every value the chart needs', () => {
    const command = helmCommand(details)

    for (const value of Object.values(details)) {
      expect(command).toContain(value)
    }
  })

  /**
   * The gap between a credential shown once and a cluster that works should
   * be one paste, so the command is a command and not a template.
   */
  it('leaves nothing to fill in', () => {
    expect(helmCommand(details)).not.toMatch(/<[^>]+>/)
  })
})
