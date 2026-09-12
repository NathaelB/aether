import { useAuthStore } from '@/stores/auth'
import { Fetcher } from './api.client'

export const fetcher: Fetcher = async (method, apiUrl, params) => {
  const headers = new Headers()

  const accessToken = useAuthStore.getState().accessToken

  // Replace path parameters (supports both {param} and :param formats)
  const actualUrl = replacePathParams(apiUrl, (params?.path ?? {}) as Record<string, string>)
  const url = new URL(actualUrl)

  // Handle query parameters
  if (params?.query) {
    const searchParams = new URLSearchParams()
    Object.entries(params.query).forEach(([key, value]) => {
      if (value != null) {
        // Skip null/undefined values
        if (Array.isArray(value)) {
          value.forEach((val) => val != null && searchParams.append(key, String(val)))
        } else {
          searchParams.append(key, String(value))
        }
      }
    })
    url.search = searchParams.toString()
  }

  // Handle request body for mutation methods
  const body = ['post', 'put', 'patch', 'delete'].includes(method.toLowerCase())
    ? JSON.stringify(params?.body)
    : undefined

  if (body) {
    headers.set('Content-Type', 'application/json')
  }

  if (accessToken) {
    headers.set('Authorization', `Bearer ${accessToken}`)
  }

  // Add custom headers
  if (params?.header) {
    Object.entries(params.header).forEach(([key, value]) => {
      if (value != null) {
        headers.set(key, String(value))
      }
    })
  }

  const response = await fetch(url, {
    method: method.toUpperCase(),
    ...(body && { body }),
    headers,
  })

  if (!response.ok) {
    throw await ApiRequestError.from(response)
  }

  return response
}

/**
 * A refused request, with what the platform said about it.
 *
 * The status is carried so callers can tell "you may not" from "it broke",
 * which decides whether retrying could ever help. The message is the
 * platform's own: it already distinguishes an expired invitation from an
 * unknown one, and from one addressed to somebody else, and rebuilding a
 * sentence here from a status code would throw all of that away.
 */
export class ApiRequestError extends Error {
  readonly status: number
  readonly code?: string

  constructor(status: number, message: string, code?: string) {
    super(message)
    this.name = 'ApiRequestError'
    this.status = status
    this.code = code
  }

  static async from(response: Response): Promise<ApiRequestError> {
    // Best effort: an error body is not guaranteed to be JSON, and a proxy
    // between here and the platform may answer with something else entirely.
    try {
      const body = await response.json()
      if (typeof body?.message === 'string' && body.message.length > 0) {
        return new ApiRequestError(response.status, body.message, body.code)
      }
    } catch {
      // Fall through to the status line.
    }

    return new ApiRequestError(
      response.status,
      `HTTP ${response.status}: ${response.statusText}`,
    )
  }

  /** Whether asking again could ever answer differently. */
  get worthRetrying(): boolean {
    // 4xx is the platform saying no to this request as sent. Asking again
    // sends the same request. 408 and 429 are the exceptions: both say "not
    // now" rather than "not this".
    if (this.status === 408 || this.status === 429) return true

    return this.status < 400 || this.status >= 500
  }
}

function replacePathParams(url: string, params: Record<string, string>): string {
  return url
    .replace(/{(\w+)}/g, (_, key: string) => params[key] || `{${key}}`)
    .replace(/:([a-zA-Z0-9_]+)/g, (_, key: string) => params[key] || `:${key}`)
}
