import { useQuery } from '@tanstack/react-query'
import type { Schemas } from '@/api/api.client'
import { selectAccessToken, useAuthStore } from '@/stores/auth'

/**
 * What this account may do to the installation, asked of the platform.
 *
 * It used to be read out of the access token: a realm role the API checked
 * too. The API holds these rights now and reads them from the database on
 * every request, so a token is no longer evidence of anything here -- and a
 * console that kept decoding one would show the section to somebody the API
 * refuses, and hide it from somebody it would let in.
 */
export const useMyPlatformRights = () => {
  const accessToken = useAuthStore(selectAccessToken)

  return useQuery({
    ...window.api.get('/platform/rights').queryOptions,
    enabled: !!accessToken,
  })
}

/**
 * Whether to draw the platform section at all.
 *
 * `undefined` while the answer is in flight, so a screen can tell "not yet"
 * from "not allowed" -- drawn as a refusal, the first frame of every page
 * accuses the reader of something before the answer arrives.
 */
export function useIsOperator(): boolean | undefined {
  const { data, isPending } = useMyPlatformRights()

  if (isPending) return undefined

  return (data?.data?.length ?? 0) > 0
}

/**
 * Whether this account holds one particular platform right.
 *
 * Asked where a screen offers something the API would refuse. Drawing the
 * control anyway and letting the request fail is a worse answer: the operator
 * has already chosen what to change by the time they are told they may not.
 */
export function useHoldsPlatformRight(right: Schemas.PlatformRight): boolean {
  const { data } = useMyPlatformRights()

  return (data?.data ?? []).includes(right)
}
