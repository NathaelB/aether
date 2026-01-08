export namespace Schemas {
  // <Schemas>
  export type ApiError =
    | 'TokenNotFound'
    | { BadRequest: { reason: string } }
    | { Unknown: { reason: string } }
    | { InternalServerError: { reason: string } }
  export type CreateOrganisationRequest = { name: string }
  export type OrganisationId = string
  export type OrganisationLimits = {
    max_instances: number
    max_storage_gb: number
    max_users: number
  }
  export type OrganisationName = string
  export type UserId = string
  export type Plan = 'Free' | 'Starter' | 'Business' | 'Enterprise'
  export type OrganisationSlug = string
  export type OrganisationStatus = 'Active' | 'Suspended' | 'Deleted'
  export type Organisation = {
    created_at: string
    deleted_at?: (string | null) | undefined
    id: OrganisationId
    limits: OrganisationLimits
    name: OrganisationName
    owner_id: UserId
    plan: Plan
    slug: OrganisationSlug
    status: OrganisationStatus
    updated_at: string
  }
  export type CreateOrganisationResponse = { data: Organisation }
  export type GetOrganisationsResponse = { data: Array<Organisation> }
  export type GetUserOrganisationsResponse = { data: Array<Organisation> }

  // </Schemas>
}

export namespace Endpoints {
  // <Endpoints>

  export type get_Get_organisations_handler = {
    method: 'GET'
    path: '/organisations'
    requestFormat: 'json'
    parameters: {
      path: { status: string | null; limit: number; offset: number }
    }
    response: Schemas.GetOrganisationsResponse
  }
  export type post_Create_organisation_handler = {
    method: 'POST'
    path: '/organisations'
    requestFormat: 'json'
    parameters: {
      body: Schemas.CreateOrganisationRequest
    }
    response: Schemas.CreateOrganisationResponse
  }
  export type get_Get_user_organisations_handler = {
    method: 'GET'
    path: '/users/@me/organisations'
    requestFormat: 'json'
    parameters: never
    response: Schemas.GetUserOrganisationsResponse
  }

  // </Endpoints>
}

// <EndpointByMethod>
export type EndpointByMethod = {
  get: {
    '/organisations': Endpoints.get_Get_organisations_handler
    '/users/@me/organisations': Endpoints.get_Get_user_organisations_handler
  }
  post: {
    '/organisations': Endpoints.post_Create_organisation_handler
  }
}

// </EndpointByMethod>

// <EndpointByMethod.Shorthands>
export type GetEndpoints = EndpointByMethod['get']
export type PostEndpoints = EndpointByMethod['post']
// </EndpointByMethod.Shorthands>

// <ApiClientTypes>
export type EndpointParameters = {
  body?: unknown
  query?: Record<string, unknown>
  header?: Record<string, unknown>
  path?: Record<string, unknown>
}

export type MutationMethod = 'post' | 'put' | 'patch' | 'delete'
export type Method = 'get' | 'head' | 'options' | MutationMethod

type RequestFormat = 'json' | 'form-data' | 'form-url' | 'binary' | 'text'

export type DefaultEndpoint = {
  parameters?: EndpointParameters | undefined
  response: unknown
  responseHeaders?: Record<string, unknown>
}

export type Endpoint<TConfig extends DefaultEndpoint = DefaultEndpoint> = {
  operationId: string
  method: Method
  path: string
  requestFormat: RequestFormat
  parameters?: TConfig['parameters']
  meta: {
    alias: string
    hasParameters: boolean
    areParametersRequired: boolean
  }
  response: TConfig['response']
  responseHeaders?: TConfig['responseHeaders']
}

export type Fetcher = (
  method: Method,
  url: string,
  parameters?: EndpointParameters | undefined
) => Promise<Response>

type RequiredKeys<T> = {
  [P in keyof T]-?: undefined extends T[P] ? never : P
}[keyof T]

type MaybeOptionalArg<T> = RequiredKeys<T> extends never ? [config?: T] : [config: T]

// </ApiClientTypes>

// <ApiClient>
export class ApiClient {
  baseUrl: string = ''

  constructor(public fetcher: Fetcher) {}

  setBaseUrl(baseUrl: string) {
    this.baseUrl = baseUrl
    return this
  }

  parseResponse = async <T>(response: Response): Promise<T> => {
    const contentType = response.headers.get('content-type')
    if (contentType?.includes('application/json')) {
      return response.json()
    }
    return response.text() as unknown as T
  }

  // <ApiClient.get>
  get<Path extends keyof GetEndpoints, TEndpoint extends GetEndpoints[Path]>(
    path: Path,
    ...params: MaybeOptionalArg<TEndpoint['parameters']>
  ): Promise<TEndpoint['response']> {
    return this.fetcher('get', this.baseUrl + path, params[0]).then((response) =>
      this.parseResponse(response)
    ) as Promise<TEndpoint['response']>
  }
  // </ApiClient.get>

  // <ApiClient.post>
  post<Path extends keyof PostEndpoints, TEndpoint extends PostEndpoints[Path]>(
    path: Path,
    ...params: MaybeOptionalArg<TEndpoint['parameters']>
  ): Promise<TEndpoint['response']> {
    return this.fetcher('post', this.baseUrl + path, params[0]).then((response) =>
      this.parseResponse(response)
    ) as Promise<TEndpoint['response']>
  }
  // </ApiClient.post>

  // <ApiClient.request>
  /**
   * Generic request method with full type-safety for any endpoint
   */
  request<
    TMethod extends keyof EndpointByMethod,
    TPath extends keyof EndpointByMethod[TMethod],
    TEndpoint extends EndpointByMethod[TMethod][TPath],
  >(
    method: TMethod,
    path: TPath,
    ...params: MaybeOptionalArg<TEndpoint extends { parameters: infer Params } ? Params : never>
  ): Promise<
    Omit<Response, 'json'> & {
      /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/Request/json) */
      json: () => Promise<TEndpoint extends { response: infer Res } ? Res : never>
    }
  > {
    return this.fetcher(method, this.baseUrl + (path as string), params[0] as EndpointParameters)
  }
  // </ApiClient.request>
}

export function createApiClient(fetcher: Fetcher, baseUrl?: string) {
  return new ApiClient(fetcher).setBaseUrl(baseUrl ?? '')
}

/**
 Example usage:
 const api = createApiClient((method, url, params) =>
   fetch(url, { method, body: JSON.stringify(params) }).then((res) => res.json()),
 );
 api.get("/users").then((users) => console.log(users));
 api.post("/users", { body: { name: "John" } }).then((user) => console.log(user));
 api.put("/users/:id", { path: { id: 1 }, body: { name: "John" } }).then((user) => console.log(user));
*/

// </ApiClient
