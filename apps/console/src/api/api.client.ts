export namespace Schemas {
  // <Schemas>
  export type ActionType = string
  export type ActionId = string
  export type ActionConstraints = Partial<{ not_after: string | null; priority: number | null }>
  export type ActionSource =
    | { User: { user_id: string } }
    | 'System'
    | { Api: { client_id: string } }
  export type ActionMetadata = {
    constraints: ActionConstraints
    created_at: string
    source: ActionSource
  }
  export type ActionPayload = { data: unknown }
  export type ActionFailureReason =
    | 'InvalidPayload'
    | 'UnsupportedAction'
    | 'PublishFailed'
    | 'Timeout'
    | { InternalError: string }
  export type ActionStatus =
    | 'Pending'
    | { Pulled: { agent_id: string; at: string } }
    | { Published: { at: string } }
    | { Failed: { at: string; reason: ActionFailureReason } }
  export type TargetKind = 'Deployment' | 'Realm' | 'Database' | 'User' | { Custom: string }
  export type ActionTarget = { id: string; kind: TargetKind }
  export type ActionVersion = number
  export type Action = {
    action_type: ActionType
    dataplane_id: string
    deployment_id: string
    id: ActionId
    metadata: ActionMetadata
    payload: ActionPayload
    status: ActionStatus
    target: ActionTarget
    version: ActionVersion
  }
  export type ApiError =
    | 'TokenNotFound'
    | { BadRequest: { reason: string } }
    | { Unknown: { reason: string } }
    | { InternalServerError: { reason: string } }
    | { Forbidden: { reason: string } }
  export type CreateDeploymentRequest = {
    kind: string
    name: string
    namespace: string
    status?: (string | null) | undefined
    version: string
  }
  export type UserId = string
  export type DeploymentId = string
  export type DeploymentKind = 'ferriskey' | 'keycloak'
  export type DeploymentName = string
  export type OrganisationId = string
  export type DeploymentStatus =
    | 'pending'
    | 'scheduling'
    | 'in_progress'
    | 'successful'
    | 'failed'
    | 'maintenance'
    | 'upgrade_required'
    | 'upgrading'
  export type DeploymentVersion = string
  export type Deployment = {
    created_at: string
    created_by: UserId
    dataplane_id: string
    deleted_at?: (string | null) | undefined
    deployed_at?: (string | null) | undefined
    id: DeploymentId
    kind: DeploymentKind
    name: DeploymentName
    namespace: string
    organisation_id: OrganisationId
    status: DeploymentStatus
    updated_at: string
    version: DeploymentVersion
  }
  export type CreateDeploymentResponse = { data: Deployment }
  export type CreateOrganisationRequest = { name: string }
  export type OrganisationLimits = {
    max_instances: number
    max_storage_gb: number
    max_users: number
  }
  export type OrganisationName = string
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
  export type CreateRoleRequest = {
    color?: (string | null) | undefined
    name: string
    permissions: number
  }
  export type RoleId = string
  export type Role = {
    color?: (string | null) | undefined
    created_at: string
    id: RoleId
    name: string
    organisation_id?: (null | OrganisationId) | undefined
    permissions: number
  }
  export type CreateRoleResponse = { data: Role }
  export type DeleteDeploymentResponse = { success: boolean }
  export type DeleteRoleResponse = { success: boolean }
  export type GetActionResponse = { data: Action }
  export type GetDeploymentResponse = { data: Deployment }
  export type GetOrganisationsResponse = { data: Array<Organisation> }
  export type GetRoleResponse = { data: Role }
  export type GetUserOrganisationsResponse = { data: Array<Organisation> }
  export type ListActionsResponse = {
    data: Array<Action>
    next_cursor?: (string | null) | undefined
  }
  export type ListDeploymentsResponse = { data: Array<Deployment> }
  export type ListRolesResponse = { data: Array<Role> }
  export type UpdateDeploymentRequest = Partial<{
    deployed_at: string | null
    kind: string | null
    name: string | null
    namespace: string | null
    status: string | null
    version: string | null
  }>
  export type UpdateDeploymentResponse = { data: Deployment }
  export type UpdateRoleRequest = Partial<{
    color: string | null
    name: string | null
    permissions: number | null
  }>
  export type UpdateRoleResponse = { data: Role }

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
  export type get_List_deployments_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/deployments'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string }
    }
    response: Schemas.ListDeploymentsResponse
  }
  export type post_Create_deployment_handler = {
    method: 'POST'
    path: '/organisations/{organisation_id}/deployments'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string }

      body: Schemas.CreateDeploymentRequest
    }
    response: Schemas.CreateDeploymentResponse
  }
  export type get_Get_deployment_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }
    }
    response: Schemas.GetDeploymentResponse
  }
  export type delete_Delete_deployment_handler = {
    method: 'DELETE'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }
    }
    response: Schemas.DeleteDeploymentResponse
  }
  export type patch_Update_deployment_handler = {
    method: 'PATCH'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }

      body: Schemas.UpdateDeploymentRequest
    }
    response: Schemas.UpdateDeploymentResponse
  }
  export type get_List_actions_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/actions'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string; cursor: string | null; limit: number }
    }
    response: Schemas.ListActionsResponse
  }
  export type get_Get_action_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/actions/{action_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string; action_id: string }
    }
    response: Schemas.GetActionResponse
  }
  export type get_List_roles_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/roles'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string }
    }
    response: Schemas.ListRolesResponse
  }
  export type post_Create_role_handler = {
    method: 'POST'
    path: '/organisations/{organisation_id}/roles'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string }

      body: Schemas.CreateRoleRequest
    }
    response: Schemas.CreateRoleResponse
  }
  export type get_Get_role_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/roles/{role_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; role_id: string }
    }
    response: Schemas.GetRoleResponse
  }
  export type delete_Delete_role_handler = {
    method: 'DELETE'
    path: '/organisations/{organisation_id}/roles/{role_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; role_id: string }
    }
    response: Schemas.DeleteRoleResponse
  }
  export type patch_Update_role_handler = {
    method: 'PATCH'
    path: '/organisations/{organisation_id}/roles/{role_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; role_id: string }

      body: Schemas.UpdateRoleRequest
    }
    response: Schemas.UpdateRoleResponse
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
    '/organisations/{organisation_id}/deployments': Endpoints.get_List_deployments_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}': Endpoints.get_Get_deployment_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/actions': Endpoints.get_List_actions_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/actions/{action_id}': Endpoints.get_Get_action_handler
    '/organisations/{organisation_id}/roles': Endpoints.get_List_roles_handler
    '/organisations/{organisation_id}/roles/{role_id}': Endpoints.get_Get_role_handler
    '/users/@me/organisations': Endpoints.get_Get_user_organisations_handler
  }
  post: {
    '/organisations': Endpoints.post_Create_organisation_handler
    '/organisations/{organisation_id}/deployments': Endpoints.post_Create_deployment_handler
    '/organisations/{organisation_id}/roles': Endpoints.post_Create_role_handler
  }
  delete: {
    '/organisations/{organisation_id}/deployments/{deployment_id}': Endpoints.delete_Delete_deployment_handler
    '/organisations/{organisation_id}/roles/{role_id}': Endpoints.delete_Delete_role_handler
  }
  patch: {
    '/organisations/{organisation_id}/deployments/{deployment_id}': Endpoints.patch_Update_deployment_handler
    '/organisations/{organisation_id}/roles/{role_id}': Endpoints.patch_Update_role_handler
  }
}

// </EndpointByMethod>

// <EndpointByMethod.Shorthands>
export type GetEndpoints = EndpointByMethod['get']
export type PostEndpoints = EndpointByMethod['post']
export type DeleteEndpoints = EndpointByMethod['delete']
export type PatchEndpoints = EndpointByMethod['patch']
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

  // <ApiClient.delete>
  delete<Path extends keyof DeleteEndpoints, TEndpoint extends DeleteEndpoints[Path]>(
    path: Path,
    ...params: MaybeOptionalArg<TEndpoint['parameters']>
  ): Promise<TEndpoint['response']> {
    return this.fetcher('delete', this.baseUrl + path, params[0]).then((response) =>
      this.parseResponse(response)
    ) as Promise<TEndpoint['response']>
  }
  // </ApiClient.delete>

  // <ApiClient.patch>
  patch<Path extends keyof PatchEndpoints, TEndpoint extends PatchEndpoints[Path]>(
    path: Path,
    ...params: MaybeOptionalArg<TEndpoint['parameters']>
  ): Promise<TEndpoint['response']> {
    return this.fetcher('patch', this.baseUrl + path, params[0]).then((response) =>
      this.parseResponse(response)
    ) as Promise<TEndpoint['response']>
  }
  // </ApiClient.patch>

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
