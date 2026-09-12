import { describe, expect, it } from 'vitest'
import { EMPTY_PUBLISH_FORM, validatePublish, type PublishForm } from './publish'

function form(over: Partial<PublishForm> = {}): PublishForm {
  return { ...EMPTY_PUBLISH_FORM, version: '26.0.1', ...over }
}

function errorsOf(result: ReturnType<typeof validatePublish>) {
  return 'errors' in result ? result.errors : {}
}

function requestOf(result: ReturnType<typeof validatePublish>) {
  if ('request' in result) return result.request
  throw new Error(`expected a request, got ${JSON.stringify(result.errors)}`)
}

describe('validatePublish', () => {
  it('sends the version and the risk', () => {
    const request = requestOf(validatePublish(form({ risk: 'breaking' })))

    expect(request.version).toBe('26.0.1')
    expect(request.risk).toBe('breaking')
  })

  /**
   * The whole reason the form checks: `latest` is the value that got typed,
   * and the API refuses it with a message about deployments recording their
   * version, which says nothing about the field it came from.
   */
  it('refuses anything that is not an exact version', () => {
    expect(errorsOf(validatePublish(form({ version: 'latest' }))).version).toContain('26.0.1')
    expect(errorsOf(validatePublish(form({ version: '26.0' }))).version).toBeDefined()
    expect(errorsOf(validatePublish(form({ version: '' }))).version).toBeDefined()
  })

  it('leaves out what was not filled in', () => {
    const request = requestOf(validatePublish(form()))

    expect(request.notes).toBeUndefined()
    expect(request.steps_through).toBeUndefined()
    expect(request.minimum_operator_version).toBeUndefined()
  })

  it('reads the stepping stones as a list', () => {
    const request = requestOf(validatePublish(form({ stepsThrough: '25.0.0, 26.0.0' })))

    expect(request.steps_through).toEqual(['25.0.0', '26.0.0'])
  })

  it('refuses a stepping stone that is not a version', () => {
    const errors = errorsOf(validatePublish(form({ stepsThrough: '25.0.0, latest' })))

    expect(errors.stepsThrough).toContain('latest')
  })

  /** The platform drops it silently, so the form is where it gets noticed. */
  it('refuses a version that steps through itself', () => {
    const errors = errorsOf(validatePublish(form({ stepsThrough: '26.0.1' })))

    expect(errors.stepsThrough).toContain('itself')
  })

  it('carries a minimum operator version when one is given', () => {
    const request = requestOf(validatePublish(form({ minimumOperatorVersion: '1.4.0' })))

    expect(request.minimum_operator_version).toBe('1.4.0')
  })

  it('refuses a minimum operator version that is not one', () => {
    expect(
      errorsOf(validatePublish(form({ minimumOperatorVersion: 'newest' }))).minimumOperatorVersion,
    ).toBeDefined()
  })
})
