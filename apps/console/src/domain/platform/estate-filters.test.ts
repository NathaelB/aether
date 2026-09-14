import { describe, expect, it } from 'vitest'
import { WHOLE_ESTATE, isNarrowed, toEstateQuery } from './estate-filters'

describe('what the estate is asked', () => {
  it('asks about everything when nothing is narrowed', () => {
    expect(toEstateQuery(WHOLE_ESTATE)).toEqual({})
    expect(isNarrowed(WHOLE_ESTATE)).toBe(false)
  })

  /**
   * The API refuses a status nobody uses, and an empty string is a status
   * nobody uses. Sent, it would turn "I cleared the filter" into a 400.
   */
  it('leaves an emptied filter out rather than sending nothing as something', () => {
    expect(toEstateQuery({ status: '', region: '   ' })).toEqual({})
  })

  it('carries every filter somebody set', () => {
    expect(
      toEstateQuery({
        organisationId: 'org',
        dataplaneId: 'dp',
        region: 'fr-par',
        status: 'successful',
      }),
    ).toEqual({
      organisation_id: 'org',
      dataplane_id: 'dp',
      region: 'fr-par',
      status: 'successful',
    })
  })

  it('knows it is narrowed as soon as one filter is set', () => {
    expect(isNarrowed({ region: 'fr-par' })).toBe(true)
    expect(isNarrowed({ region: '' })).toBe(false)
  })
})
