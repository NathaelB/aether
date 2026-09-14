/**
 * What somebody narrowed the estate to.
 *
 * Every field optional and empty meaning "everything", so the screen's initial
 * state and "clear the filters" are the same value rather than two.
 */
export interface EstateFilters {
  organisationId?: string
  dataplaneId?: string
  region?: string
  status?: string
}

/** Nothing narrowed: the whole estate. */
export const WHOLE_ESTATE: EstateFilters = {}

/**
 * The query the platform is asked.
 *
 * An empty filter is left out rather than sent as an empty string: the API
 * refuses a status nobody uses, and `status=''` is a status nobody uses.
 */
export function toEstateQuery(filters: EstateFilters): Record<string, string> {
  const query: Record<string, string> = {}

  const carry = (key: string, value: string | undefined) => {
    const trimmed = value?.trim()
    if (trimmed) query[key] = trimmed
  }

  carry('organisation_id', filters.organisationId)
  carry('dataplane_id', filters.dataplaneId)
  carry('region', filters.region)
  carry('status', filters.status)

  return query
}

/** Whether anything at all is narrowed, for a "clear" control that knows. */
export function isNarrowed(filters: EstateFilters): boolean {
  return Object.keys(toEstateQuery(filters)).length > 0
}
