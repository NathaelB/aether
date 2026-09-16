export namespace Schemas {
  // <Schemas>
  export type AcceptInvitationRequest = { token: string }
  export type MemberId = string
  export type UserId = string
  export type OrganisationId = string
  export type RoleId = string
  export type Role = {
    color?: (string | null) | undefined
    created_at: string
    id: RoleId
    name: string
    organisation_id?: (null | OrganisationId) | undefined
    permissions: number
  }
  export type Member = {
    email: string
    id: MemberId
    invited_by?: (null | UserId) | undefined
    joined_at: string
    name: string
    organisation_id: OrganisationId
    roles: Array<Role>
    user_id: UserId
  }
  export type AcceptInvitationResponse = { data: Member }
  export type ActionFailureReason =
    | 'InvalidPayload'
    | 'UnsupportedAction'
    | 'PublishFailed'
    | 'Timeout'
    | { InternalError: string }
  export type AckActionsFailedItem = { action_id: string; reason: ActionFailureReason }
  export type AckActionsRequest = { failed: Array<AckActionsFailedItem>; published: Array<string> }
  export type AckActionsResponseData = { acknowledged: number }
  export type AckActionsResponse = { data: AckActionsResponseData }
  export type ActionType = string
  export type DataPlaneId = string
  export type DeploymentId = string
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
  export type ActionStatus =
    | 'Pending'
    | { Leased: { until: string } }
    | { Pulled: { agent_id: string; at: string } }
    | { Published: { at: string } }
    | { Failed: { at: string; reason: ActionFailureReason } }
  export type TargetKind = 'Deployment' | 'Realm' | 'Database' | 'User' | { Custom: string }
  export type ActionTarget = { id: string; kind: TargetKind }
  export type ActionVersion = number
  export type Action = {
    action_type: ActionType
    dataplane_id: DataPlaneId
    deployment_id: DeploymentId
    id: ActionId
    leased_until?: (string | null) | undefined
    metadata: ActionMetadata
    payload: ActionPayload
    status: ActionStatus
    target: ActionTarget
    version: ActionVersion
  }
  export type AllowList = Array<string>
  export type ApiError =
    | 'TokenNotFound'
    | { BadRequest: { reason: string } }
    | { Unknown: { reason: string } }
    | { InternalServerError: { reason: string } }
    | { Forbidden: { reason: string } }
    | { Conflict: { reason: string } }
    | { NotFound: { reason: string } }
  export type KeyName = string
  export type ProviderName = string
  export type KeyVersion = number
  export type KeyRef = { name: KeyName; provider: ProviderName; version: KeyVersion }
  export type ArchiveProtection = { kind: 'store_managed' } | (KeyRef & { kind: 'envelope' })
  export type AuditAction = string
  export type AuditActor = { User: { user_id: string } } | 'System' | { Api: { client_id: string } }
  export type AuditChange = { after: unknown; before: unknown }
  export type AuditEntryId = string
  export type AuditTargetKind =
    | 'Organisation'
    | 'Deployment'
    | 'DataPlane'
    | 'Role'
    | 'Member'
    | 'Billing'
    | { Custom: string }
  export type AuditTarget = { id: string; kind: AuditTargetKind }
  export type AuditEntry = {
    action: AuditAction
    actor: AuditActor
    change?: (null | AuditChange) | undefined
    id: AuditEntryId
    organisation_id: OrganisationId
    recorded_at: string
    target: AuditTarget
  }
  export type AutoUpgradePolicy = 'manual' | 'patch' | 'patch_and_minor'
  export type BackupId = string
  export type ObjectLocation = string
  export type BackupMethod = 'logical' | 'physical'
  export type PostgresMajor = number
  export type DeploymentKind = 'ferriskey' | 'keycloak'
  export type Version = string
  export type ReleaseId = { kind: DeploymentKind; version: Version }
  export type Backup = {
    deployment_id: DeploymentId
    finished_at: string
    id: BackupId
    location: ObjectLocation
    method: BackupMethod
    organisation_id: OrganisationId
    postgres_major: PostgresMajor
    protection: ArchiveProtection
    release: ReleaseId
    server_name?: (string | null) | undefined
    size_bytes: number
    started_at: string
  }
  export type Cadence =
    | { at: string; every: 'daily' }
    | { at: string; day: string; every: 'weekly' }
  export type Retention = { keep_for_days: number; keep_last: number }
  export type BackupSchedule = {
    cadence: Cadence
    created_at: string
    deployment_id: DeploymentId
    enabled: boolean
    method: BackupMethod
    organisation_id: OrganisationId
    retention: Retention
    updated_at: string
    zone: string
  }
  export type BackupScheduleResponse = { data: BackupSchedule }
  export type BreakingRisk = 'none' | 'config' | 'breaking'
  export type Capacity = { cpu_millis: number; memory_mib: number; storage_gib: number }
  export type ClaimActionsRequest = { lease_seconds: number; max: number }
  export type ClaimActionsResponse = { data: Array<Action> }
  export type DataPlaneMode = 'shared' | 'dedicated'
  export type Region = string
  export type CreateDataPlaneRequest = {
    capacity: Capacity
    mode: DataPlaneMode
    organisation_id?: (null | OrganisationId) | undefined
    region: Region
  }
  export type CreateDeploymentRequest = {
    environment: string
    kind: string
    name: string
    offer: string
    region?: (string | null) | undefined
    version: string
  }
  export type Environment = 'production' | 'staging' | 'development'
  export type MaintenanceWindow = { day: string; duration: number; start: string; timezone: string }
  export type DeploymentName = string
  export type NetworkAccess = { kind: 'open' } | { allowed: AllowList; kind: 'restricted' }
  export type Offer = 'sandbox' | 'standard' | 'scale' | 'private'
  export type DeploymentResources = { cpu_millis: number; memory_mib: number; storage_gib: number }
  export type DeploymentStatus =
    | 'pending'
    | 'scheduling'
    | 'in_progress'
    | 'successful'
    | 'failed'
    | 'maintenance'
    | 'upgrade_required'
    | 'upgrading'
    | 'deleting'
    | 'deleted'
  export type Deployment = {
    auto_upgrade: AutoUpgradePolicy
    created_at: string
    created_by: UserId
    dataplane_id: DataPlaneId
    deleted_at?: (string | null) | undefined
    deployed_at?: (string | null) | undefined
    environment: Environment
    id: DeploymentId
    kind: DeploymentKind
    maintenance_window?: (null | MaintenanceWindow) | undefined
    name: DeploymentName
    namespace: string
    network_access: NetworkAccess
    offer?: (null | Offer) | undefined
    organisation_id: OrganisationId
    resources: DeploymentResources
    restored_from?: (null | BackupId) | undefined
    status: DeploymentStatus
    updated_at: string
    version: Version
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
  export type CreateRoleResponse = { data: Role }
  export type DataPlaneAllocation = 'shared' | { dedicated: { organisation_id: OrganisationId } }
  export type HeraldBinding = { client_id: string; subject: string }
  export type DataPlaneStatus = 'provisioning' | 'active' | 'draining' | 'disabled' | 'failed'
  export type DataPlane = {
    allocation: DataPlaneAllocation
    capacity: Capacity
    created_at: string
    herald?: (null | HeraldBinding) | undefined
    id: DataPlaneId
    last_seen_at?: (string | null) | undefined
    operator_version?: (null | Version) | undefined
    region: Region
    status: DataPlaneStatus
  }
  export type DataPlaneResponse = { data: DataPlane }
  export type DeleteDeploymentResponse = { success: boolean }
  export type DeleteRoleResponse = { success: boolean }
  export type Ending = 'finished' | 'unreadable'
  export type EstateOwner = { id: OrganisationId; name: string }
  export type EstateDeployment = {
    deployment: Deployment
    organisation: EstateOwner
    region: Region
  }
  export type EstateDeploymentsResponse = {
    data: Array<EstateDeployment>
    next_cursor?: (null | DeploymentId) | undefined
  }
  export type GetActionResponse = { data: Action }
  export type GetActiveUsersResponseData = Partial<{ active_users: number | null }>
  export type GetActiveUsersResponse = { data: GetActiveUsersResponseData }
  export type GetDataPlaneResponse = { data: DataPlane }
  export type GetDeploymentResponse = { data: Deployment }
  export type UsageBucketResponse = { bucket: string; value: number }
  export type GetDeploymentUsageResponse = { data: Array<UsageBucketResponse> }
  export type GetRoleResponse = { data: Role }
  export type GetUserOrganisationsResponse = { data: Array<Organisation> }
  export type GrantOperatorRequest = { rights: Array<string> }
  export type HeartbeatRequest = Partial<{ operator_version: string | null }>
  export type HeartbeatResponseData = { recorded: boolean }
  export type HeartbeatResponse = { data: HeartbeatResponseData }
  export type HeldBackDataPlane = {
    id: DataPlaneId
    operator_version?: (null | Version) | undefined
  }
  export type InFlightUpgrade = {
    current: Version
    from: Version
    started_at: string
    steps: Array<Version>
    target: Version
  }
  export type ReleaseStatus = 'upcoming' | 'available' | 'deprecated' | 'withdrawn'
  export type IneligibilityReason =
    | { kind: 'not_installable'; status: ReleaseStatus }
    | { dataplane?: (null | Version) | undefined; kind: 'operator_too_old'; minimum: Version }
    | { kind: 'outside_rollout' }
  export type InvitedEmail = string
  export type InvitationId = string
  export type Invitation = {
    accepted_at?: (string | null) | undefined
    created_at: string
    email: InvitedEmail
    expires_at: string
    id: InvitationId
    invited_by?: (null | UserId) | undefined
    organisation_id: OrganisationId
    revoked_at?: (string | null) | undefined
    roles: Array<Role>
  }
  export type InvitationResponse = { data: Invitation }
  export type InviteRequest = { email: string; roles?: Array<string> | undefined }
  export type InviteResponse = { data: Invitation; token: string }
  export type ListActionsResponse = {
    data: Array<Action>
    next_cursor?: (string | null) | undefined
  }
  export type ListAuditLogResponse = {
    data: Array<AuditEntry>
    next_cursor?: (string | null) | undefined
  }
  export type ListBackupsResponse = { data: Array<Backup> }
  export type ListDataplanesResponse = { data: Array<DataPlane> }
  export type ListDeploymentsForDataPlaneResponse = { data: Array<Deployment> }
  export type ListDeploymentsResponse = { data: Array<Deployment> }
  export type ListInvitationsResponse = { data: Array<Invitation> }
  export type ListMembersResponse = { data: Array<Member> }
  export type OfferAvailability = {
    offer: Offer
    open: boolean
    opened_by?: (null | Plan) | undefined
    resources: DeploymentResources
    shares_a_cluster: boolean
  }
  export type ListOffersResponse = { data: Array<OfferAvailability> }
  export type ListRegionsResponse = { data: Array<Region> }
  export type ReleaseNotes = string
  export type RolloutPercentage = number
  export type Rollout = {
    percentage: RolloutPercentage
    pilot_organisations: Array<OrganisationId>
    plans?: (Array<Plan> | null) | undefined
  }
  export type Release = {
    created_at: string
    id: ReleaseId
    minimum_operator_version?: (null | Version) | undefined
    notes: ReleaseNotes
    risk: BreakingRisk
    rollout: Rollout
    status: ReleaseStatus
    steps_through: Array<Version>
    updated_at: string
  }
  export type ReleaseInUse = Release & { deployments: number }
  export type ListReleasesInUseResponse = { data: Array<ReleaseInUse> }
  export type ListReleasesResponse = { data: Array<Release> }
  export type ListRolesResponse = { data: Array<Role> }
  export type LogLine = { at: string; message: string; source: string }
  export type MaintenanceWindowRequest = {
    day: string
    minutes: number
    start: string
    timezone: string
  }
  export type MemberResponse = { data: Member }
  export type MoveReleaseRequest = { status: ReleaseStatus }
  export type MoveTenantPlanRequest = { plan: string }
  export type MyPermissions = { permissions: number }
  export type MyPermissionsResponse = { data: MyPermissions }
  export type PlatformRight = 'view_estate' | 'operate_fleet' | 'act_on_tenant' | 'manage_operators'
  export type PlatformRights = Array<PlatformRight>
  export type MyRightsResponse = { data: PlatformRights }
  export type NetworkAccessResponse = { data: NetworkAccess }
  export type PlatformOperator = {
    granted_at: string
    granted_by?: (string | null) | undefined
    rights: PlatformRights
    subject: string
  }
  export type OperatorResponse = { data: PlatformOperator }
  export type OperatorsResponse = { data: Array<PlatformOperator> }
  export type PublishReleaseRequest = {
    minimum_operator_version?: (string | null) | undefined
    notes?: string | undefined
    risk: BreakingRisk
    steps_through?: Array<string> | undefined
    version: string
  }
  export type PushLogsRequest = { ending?: (null | Ending) | undefined; lines: Array<LogLine> }
  export type PushLogsResponseData = { listening: boolean; relayed: number }
  export type PushLogsResponse = { data: PushLogsResponseData }
  export type RegisteredDataPlaneResponse = DataPlane &
    Partial<{ herald_client_id: string | null; herald_secret: string | null }>
  export type ReleaseAvailability = Release & {
    eligible: boolean
    reason?: (null | IneligibilityReason) | undefined
  }
  export type ReleaseAvailabilityResponse = { data: Array<ReleaseAvailability> }
  export type ReleaseHoldBacksResponse = { data: Array<HeldBackDataPlane> }
  export type ReleaseResponse = { data: Release }
  export type ReportArchiveRequest = Partial<{
    error: string | null
    finished_at: string | null
    key_name: string | null
    key_provider: string | null
    key_version: number | null
    method: string | null
    object_key: string | null
    postgres_major: number | null
    server_name: string | null
    size_bytes: string | null
    started_at: string | null
  }>
  export type ReportArchiveResponseData = Partial<{ backup_id: string | null }>
  export type ReportArchiveResponse = { data: ReportArchiveResponseData }
  export type ReportOutcomeRequest = { outcome: string; version?: (string | null) | undefined }
  export type ReportOutcomeResponseData = { recorded: boolean }
  export type ReportOutcomeResponse = { data: ReportOutcomeResponseData }
  export type ReportedMetricPoint = { bucket: string; metric: string; value: number }
  export type ReportUsageMetricsRequest = { points: Array<ReportedMetricPoint> }
  export type ReportUsageMetricsResponseData = { recorded: number }
  export type ReportUsageMetricsResponse = { data: ReportUsageMetricsResponseData }
  export type RestoreBackupRequest = { name: string; region?: (string | null) | undefined }
  export type RestoreBackupResponse = { data: Deployment }
  export type ReviseReleaseRequest = {
    minimum_operator_version?: (string | null) | undefined
    notes?: string | undefined
    risk: BreakingRisk
    steps_through?: Array<string> | undefined
  }
  export type RolloutCoverage = { covered: number; total: number }
  export type RolloutCoverageResponse = { data: RolloutCoverage }
  export type RolloutRequest = {
    percentage: number
    pilot_organisations?: Array<string> | undefined
    plans?: (Array<string> | null) | undefined
  }
  export type ServiceIntent = 'draining' | 'disabled' | 'in_service'
  export type SetBackupScheduleRequest = {
    at: string
    day?: (string | null) | undefined
    enabled: boolean
    every: string
    keep_for_days: number
    keep_last: number
    zone: string
  }
  export type SetMemberRolesRequest = Partial<{ roles: Array<string> }>
  export type SetMemberRolesResponse = { data: Member }
  export type SetNetworkAccessRequest = Partial<{ allowed_cidrs: Array<string> }>
  export type SetNetworkAccessResponse = { data: Deployment }
  export type SetServiceRequest = { service: ServiceIntent }
  export type SetUpgradeSettingsRequest = {
    auto_upgrade: AutoUpgradePolicy
    maintenance_window?: (null | MaintenanceWindowRequest) | undefined
  }
  export type Tenant = { deployments: number; members: number; organisation: Organisation }
  export type TenantResponse = { data: Tenant }
  export type TenantsResponse = {
    data: Array<Tenant>
    next_cursor?: (null | OrganisationId) | undefined
  }
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
  export type UpgradeDeploymentRequest = { version: string }
  export type UpgradeDeploymentResponse = { change: string; data: Deployment }
  export type UpgradeInFlightResponse = Partial<{ data: null | InFlightUpgrade }>
  export type UpgradeSettingsResponse = { data: Deployment }

  // </Schemas>
}

export namespace Endpoints {
  // <Endpoints>

  export type get_List_dataplanes_handler = {
    method: 'GET'
    path: '/dataplanes'
    requestFormat: 'json'
    parameters: never
    response: Schemas.ListDataplanesResponse
  }
  export type post_Create_dataplane_handler = {
    method: 'POST'
    path: '/dataplanes'
    requestFormat: 'json'
    parameters: {
      body: Schemas.CreateDataPlaneRequest
    }
    response: Schemas.RegisteredDataPlaneResponse
  }
  export type get_Get_dataplane_handler = {
    method: 'GET'
    path: '/dataplanes/{dataplane_id}'
    requestFormat: 'json'
    parameters: {
      path: { dataplane_id: string }
    }
    response: Schemas.GetDataPlaneResponse
  }
  export type post_Reissue_herald_credential_handler = {
    method: 'POST'
    path: '/dataplanes/{dataplane_id}/credential'
    requestFormat: 'json'
    parameters: {
      path: { dataplane_id: string }
    }
    response: Schemas.RegisteredDataPlaneResponse
  }
  export type get_List_deployments_for_dataplane_handler = {
    method: 'GET'
    path: '/dataplanes/{dataplane_id}/deployments'
    requestFormat: 'json'
    parameters: {
      query: Partial<{ shard_index: number; shard_count: number; limit: number; cursor: string }>
      path: { dataplane_id: string }
    }
    response: Schemas.ListDeploymentsForDataPlaneResponse
  }
  export type post_Ack_actions_handler = {
    method: 'POST'
    path: '/dataplanes/{dataplane_id}/deployments/{deployment_id}/actions:ack'
    requestFormat: 'json'
    parameters: {
      path: { dataplane_id: string; deployment_id: string }

      body: Schemas.AckActionsRequest
    }
    response: Schemas.AckActionsResponse
  }
  export type post_Claim_actions_handler = {
    method: 'POST'
    path: '/dataplanes/{dataplane_id}/deployments/{deployment_id}/actions:claim'
    requestFormat: 'json'
    parameters: {
      path: { dataplane_id: string; deployment_id: string }

      body: Schemas.ClaimActionsRequest
    }
    response: Schemas.ClaimActionsResponse
  }
  export type post_Report_archive_handler = {
    method: 'POST'
    path: '/dataplanes/{dataplane_id}/deployments/{deployment_id}/archive'
    requestFormat: 'json'
    parameters: {
      path: { dataplane_id: string; deployment_id: string }

      body: Schemas.ReportArchiveRequest
    }
    response: Schemas.ReportArchiveResponse
  }
  export type post_Push_logs_handler = {
    method: 'POST'
    path: '/dataplanes/{dataplane_id}/deployments/{deployment_id}/logs/{session_id}'
    requestFormat: 'json'
    parameters: {
      path: { dataplane_id: string; deployment_id: string; session_id: string }

      body: Schemas.PushLogsRequest
    }
    response: Schemas.PushLogsResponse
  }
  export type post_Report_outcome_handler = {
    method: 'POST'
    path: '/dataplanes/{dataplane_id}/deployments/{deployment_id}/outcome'
    requestFormat: 'json'
    parameters: {
      path: { dataplane_id: string; deployment_id: string }

      body: Schemas.ReportOutcomeRequest
    }
    response: Schemas.ReportOutcomeResponse
  }
  export type post_Heartbeat_handler = {
    method: 'POST'
    path: '/dataplanes/{dataplane_id}/heartbeat'
    requestFormat: 'json'
    parameters: {
      path: { dataplane_id: string }

      body: Schemas.HeartbeatRequest
    }
    response: Schemas.HeartbeatResponse
  }
  export type put_Set_service_handler = {
    method: 'PUT'
    path: '/dataplanes/{dataplane_id}/service'
    requestFormat: 'json'
    parameters: {
      path: { dataplane_id: string }

      body: Schemas.SetServiceRequest
    }
    response: Schemas.DataPlaneResponse
  }
  export type post_Report_usage_metrics_handler = {
    method: 'POST'
    path: '/deployments/{deployment_id}/usage-metrics'
    requestFormat: 'json'
    parameters: {
      path: { deployment_id: string }

      body: Schemas.ReportUsageMetricsRequest
    }
    response: Schemas.ReportUsageMetricsResponse
  }
  export type post_Accept_invitation_handler = {
    method: 'POST'
    path: '/invitations/accept'
    requestFormat: 'json'
    parameters: {
      body: Schemas.AcceptInvitationRequest
    }
    response: Schemas.AcceptInvitationResponse
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
  export type get_List_audit_log_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/audit-log'
    requestFormat: 'json'
    parameters: {
      query: Partial<{ cursor: string; limit: number }>
      path: { organisation_id: string }
    }
    response: Schemas.ListAuditLogResponse
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
      query: Partial<{ cursor: string; limit: number }>
      path: { organisation_id: string; deployment_id: string }
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
  export type get_Get_active_users_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/active-users'
    requestFormat: 'json'
    parameters: {
      query: { window_minutes: number }
      path: { organisation_id: string; deployment_id: string }
    }
    response: Schemas.GetActiveUsersResponse
  }
  export type get_Get_backup_schedule_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/backup-schedule'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }
    }
    response: Schemas.BackupScheduleResponse
  }
  export type put_Set_backup_schedule_handler = {
    method: 'PUT'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/backup-schedule'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }

      body: Schemas.SetBackupScheduleRequest
    }
    response: Schemas.BackupScheduleResponse
  }
  export type get_List_backups_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/backups'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }
    }
    response: Schemas.ListBackupsResponse
  }
  export type post_Ask_for_backup_handler = {
    method: 'POST'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/backups'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }
    }
    response: unknown
  }
  export type post_Restore_backup_handler = {
    method: 'POST'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/backups/{backup_id}/restore'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string; backup_id: string }

      body: Schemas.RestoreBackupRequest
    }
    response: Schemas.RestoreBackupResponse
  }
  export type get_Read_logs_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/logs'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string; since_minutes: number }
    }
    response: unknown
  }
  export type get_Get_network_access_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/network-access'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }
    }
    response: Schemas.NetworkAccessResponse
  }
  export type put_Set_network_access_handler = {
    method: 'PUT'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/network-access'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }

      body: Schemas.SetNetworkAccessRequest
    }
    response: Schemas.SetNetworkAccessResponse
  }
  export type get_Upgrade_in_flight_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/upgrade'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }
    }
    response: Schemas.UpgradeInFlightResponse
  }
  export type post_Upgrade_deployment_handler = {
    method: 'POST'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/upgrade'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }

      body: Schemas.UpgradeDeploymentRequest
    }
    response: Schemas.UpgradeDeploymentResponse
  }
  export type put_Set_upgrade_settings_handler = {
    method: 'PUT'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/upgrade-settings'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }

      body: Schemas.SetUpgradeSettingsRequest
    }
    response: Schemas.UpgradeSettingsResponse
  }
  export type get_Get_deployment_usage_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/deployments/{deployment_id}/usage-metrics/{metric}'
    requestFormat: 'json'
    parameters: {
      query: { from: string; until: string }
      path: { organisation_id: string; deployment_id: string; metric: string }
    }
    response: Schemas.GetDeploymentUsageResponse
  }
  export type get_List_invitations_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/invitations'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string }
    }
    response: Schemas.ListInvitationsResponse
  }
  export type post_Invite_handler = {
    method: 'POST'
    path: '/organisations/{organisation_id}/invitations'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string }

      body: Schemas.InviteRequest
    }
    response: Schemas.InviteResponse
  }
  export type delete_Revoke_invitation_handler = {
    method: 'DELETE'
    path: '/organisations/{organisation_id}/invitations/{invitation_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; invitation_id: string }
    }
    response: Schemas.InvitationResponse
  }
  export type get_List_members_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/members'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string }
    }
    response: Schemas.ListMembersResponse
  }
  export type get_Get_member_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/members/{user_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; user_id: string }
    }
    response: Schemas.MemberResponse
  }
  export type delete_Remove_member_handler = {
    method: 'DELETE'
    path: '/organisations/{organisation_id}/members/{user_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; user_id: string }
    }
    response: unknown
  }
  export type put_Set_member_roles_handler = {
    method: 'PUT'
    path: '/organisations/{organisation_id}/members/{user_id}/roles'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; user_id: string }

      body: Schemas.SetMemberRolesRequest
    }
    response: Schemas.SetMemberRolesResponse
  }
  export type get_List_offers_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/offers'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string }
    }
    response: Schemas.ListOffersResponse
  }
  export type get_My_permissions_handler = {
    method: 'GET'
    path: '/organisations/{organisation_id}/permissions'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string }
    }
    response: Schemas.MyPermissionsResponse
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
  export type get_List_estate_deployments_handler = {
    method: 'GET'
    path: '/platform/deployments'
    requestFormat: 'json'
    parameters: {
      query: Partial<{
        organisation_id: string
        dataplane_id: string
        region: string
        status: string
        limit: number
        cursor: string
      }>
    }
    response: Schemas.EstateDeploymentsResponse
  }
  export type get_List_operators_handler = {
    method: 'GET'
    path: '/platform/operators'
    requestFormat: 'json'
    parameters: never
    response: Schemas.OperatorsResponse
  }
  export type put_Grant_operator_handler = {
    method: 'PUT'
    path: '/platform/operators/{subject}'
    requestFormat: 'json'
    parameters: {
      path: { subject: string }

      body: Schemas.GrantOperatorRequest
    }
    response: Schemas.OperatorResponse
  }
  export type delete_Revoke_operator_handler = {
    method: 'DELETE'
    path: '/platform/operators/{subject}'
    requestFormat: 'json'
    parameters: {
      path: { subject: string }
    }
    response: unknown
  }
  export type get_List_tenants_handler = {
    method: 'GET'
    path: '/platform/organisations'
    requestFormat: 'json'
    parameters: {
      query: Partial<{ status: string; limit: number; cursor: string }>
    }
    response: Schemas.TenantsResponse
  }
  export type get_Get_tenant_handler = {
    method: 'GET'
    path: '/platform/organisations/{organisation_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string }
    }
    response: Schemas.TenantResponse
  }
  export type put_Move_tenant_plan_handler = {
    method: 'PUT'
    path: '/platform/organisations/{organisation_id}/plan'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string }

      body: Schemas.MoveTenantPlanRequest
    }
    response: Schemas.TenantResponse
  }
  export type get_My_rights_handler = {
    method: 'GET'
    path: '/platform/rights'
    requestFormat: 'json'
    parameters: never
    response: Schemas.MyRightsResponse
  }
  export type get_List_regions_handler = {
    method: 'GET'
    path: '/regions'
    requestFormat: 'json'
    parameters: never
    response: Schemas.ListRegionsResponse
  }
  export type get_Release_availability_handler = {
    method: 'GET'
    path: '/releases/deployments/{organisation_id}/{deployment_id}'
    requestFormat: 'json'
    parameters: {
      path: { organisation_id: string; deployment_id: string }
    }
    response: Schemas.ReleaseAvailabilityResponse
  }
  export type get_List_releases_for_operator_handler = {
    method: 'GET'
    path: '/releases/operator/{kind}'
    requestFormat: 'json'
    parameters: {
      path: { kind: 'ferriskey' | 'keycloak' }
    }
    response: Schemas.ListReleasesInUseResponse
  }
  export type post_Publish_release_handler = {
    method: 'POST'
    path: '/releases/operator/{kind}'
    requestFormat: 'json'
    parameters: {
      path: { kind: 'ferriskey' | 'keycloak' }

      body: Schemas.PublishReleaseRequest
    }
    response: Schemas.ReleaseResponse
  }
  export type patch_Revise_release_handler = {
    method: 'PATCH'
    path: '/releases/operator/{kind}/{version}'
    requestFormat: 'json'
    parameters: {
      path: { kind: 'ferriskey' | 'keycloak'; version: string }

      body: Schemas.ReviseReleaseRequest
    }
    response: Schemas.ReleaseResponse
  }
  export type get_Release_hold_backs_handler = {
    method: 'GET'
    path: '/releases/operator/{kind}/{version}/hold-backs'
    requestFormat: 'json'
    parameters: {
      path: { kind: 'ferriskey' | 'keycloak'; version: string }
    }
    response: Schemas.ReleaseHoldBacksResponse
  }
  export type put_Widen_rollout_handler = {
    method: 'PUT'
    path: '/releases/operator/{kind}/{version}/rollout'
    requestFormat: 'json'
    parameters: {
      path: { kind: 'ferriskey' | 'keycloak'; version: string }

      body: Schemas.RolloutRequest
    }
    response: Schemas.ReleaseResponse
  }
  export type post_Preview_rollout_coverage_handler = {
    method: 'POST'
    path: '/releases/operator/{kind}/{version}/rollout/preview'
    requestFormat: 'json'
    parameters: {
      path: { kind: 'ferriskey' | 'keycloak'; version: string }

      body: Schemas.RolloutRequest
    }
    response: Schemas.RolloutCoverageResponse
  }
  export type put_Move_release_handler = {
    method: 'PUT'
    path: '/releases/operator/{kind}/{version}/status'
    requestFormat: 'json'
    parameters: {
      path: { kind: 'ferriskey' | 'keycloak'; version: string }

      body: Schemas.MoveReleaseRequest
    }
    response: Schemas.ReleaseResponse
  }
  export type get_List_releases_handler = {
    method: 'GET'
    path: '/releases/{kind}'
    requestFormat: 'json'
    parameters: {
      path: { kind: 'ferriskey' | 'keycloak' }
    }
    response: Schemas.ListReleasesResponse
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
    '/dataplanes': Endpoints.get_List_dataplanes_handler
    '/dataplanes/{dataplane_id}': Endpoints.get_Get_dataplane_handler
    '/dataplanes/{dataplane_id}/deployments': Endpoints.get_List_deployments_for_dataplane_handler
    '/organisations/{organisation_id}/audit-log': Endpoints.get_List_audit_log_handler
    '/organisations/{organisation_id}/deployments': Endpoints.get_List_deployments_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}': Endpoints.get_Get_deployment_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/actions': Endpoints.get_List_actions_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/actions/{action_id}': Endpoints.get_Get_action_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/active-users': Endpoints.get_Get_active_users_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/backup-schedule': Endpoints.get_Get_backup_schedule_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/backups': Endpoints.get_List_backups_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/logs': Endpoints.get_Read_logs_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/network-access': Endpoints.get_Get_network_access_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/upgrade': Endpoints.get_Upgrade_in_flight_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/usage-metrics/{metric}': Endpoints.get_Get_deployment_usage_handler
    '/organisations/{organisation_id}/invitations': Endpoints.get_List_invitations_handler
    '/organisations/{organisation_id}/members': Endpoints.get_List_members_handler
    '/organisations/{organisation_id}/members/{user_id}': Endpoints.get_Get_member_handler
    '/organisations/{organisation_id}/offers': Endpoints.get_List_offers_handler
    '/organisations/{organisation_id}/permissions': Endpoints.get_My_permissions_handler
    '/organisations/{organisation_id}/roles': Endpoints.get_List_roles_handler
    '/organisations/{organisation_id}/roles/{role_id}': Endpoints.get_Get_role_handler
    '/platform/deployments': Endpoints.get_List_estate_deployments_handler
    '/platform/operators': Endpoints.get_List_operators_handler
    '/platform/organisations': Endpoints.get_List_tenants_handler
    '/platform/organisations/{organisation_id}': Endpoints.get_Get_tenant_handler
    '/platform/rights': Endpoints.get_My_rights_handler
    '/regions': Endpoints.get_List_regions_handler
    '/releases/deployments/{organisation_id}/{deployment_id}': Endpoints.get_Release_availability_handler
    '/releases/operator/{kind}': Endpoints.get_List_releases_for_operator_handler
    '/releases/operator/{kind}/{version}/hold-backs': Endpoints.get_Release_hold_backs_handler
    '/releases/{kind}': Endpoints.get_List_releases_handler
    '/users/@me/organisations': Endpoints.get_Get_user_organisations_handler
  }
  post: {
    '/dataplanes': Endpoints.post_Create_dataplane_handler
    '/dataplanes/{dataplane_id}/credential': Endpoints.post_Reissue_herald_credential_handler
    '/dataplanes/{dataplane_id}/deployments/{deployment_id}/actions:ack': Endpoints.post_Ack_actions_handler
    '/dataplanes/{dataplane_id}/deployments/{deployment_id}/actions:claim': Endpoints.post_Claim_actions_handler
    '/dataplanes/{dataplane_id}/deployments/{deployment_id}/archive': Endpoints.post_Report_archive_handler
    '/dataplanes/{dataplane_id}/deployments/{deployment_id}/logs/{session_id}': Endpoints.post_Push_logs_handler
    '/dataplanes/{dataplane_id}/deployments/{deployment_id}/outcome': Endpoints.post_Report_outcome_handler
    '/dataplanes/{dataplane_id}/heartbeat': Endpoints.post_Heartbeat_handler
    '/deployments/{deployment_id}/usage-metrics': Endpoints.post_Report_usage_metrics_handler
    '/invitations/accept': Endpoints.post_Accept_invitation_handler
    '/organisations': Endpoints.post_Create_organisation_handler
    '/organisations/{organisation_id}/deployments': Endpoints.post_Create_deployment_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/backups': Endpoints.post_Ask_for_backup_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/backups/{backup_id}/restore': Endpoints.post_Restore_backup_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/upgrade': Endpoints.post_Upgrade_deployment_handler
    '/organisations/{organisation_id}/invitations': Endpoints.post_Invite_handler
    '/organisations/{organisation_id}/roles': Endpoints.post_Create_role_handler
    '/releases/operator/{kind}': Endpoints.post_Publish_release_handler
    '/releases/operator/{kind}/{version}/rollout/preview': Endpoints.post_Preview_rollout_coverage_handler
  }
  put: {
    '/dataplanes/{dataplane_id}/service': Endpoints.put_Set_service_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/backup-schedule': Endpoints.put_Set_backup_schedule_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/network-access': Endpoints.put_Set_network_access_handler
    '/organisations/{organisation_id}/deployments/{deployment_id}/upgrade-settings': Endpoints.put_Set_upgrade_settings_handler
    '/organisations/{organisation_id}/members/{user_id}/roles': Endpoints.put_Set_member_roles_handler
    '/platform/operators/{subject}': Endpoints.put_Grant_operator_handler
    '/platform/organisations/{organisation_id}/plan': Endpoints.put_Move_tenant_plan_handler
    '/releases/operator/{kind}/{version}/rollout': Endpoints.put_Widen_rollout_handler
    '/releases/operator/{kind}/{version}/status': Endpoints.put_Move_release_handler
  }
  delete: {
    '/organisations/{organisation_id}/deployments/{deployment_id}': Endpoints.delete_Delete_deployment_handler
    '/organisations/{organisation_id}/invitations/{invitation_id}': Endpoints.delete_Revoke_invitation_handler
    '/organisations/{organisation_id}/members/{user_id}': Endpoints.delete_Remove_member_handler
    '/organisations/{organisation_id}/roles/{role_id}': Endpoints.delete_Delete_role_handler
    '/platform/operators/{subject}': Endpoints.delete_Revoke_operator_handler
  }
  patch: {
    '/organisations/{organisation_id}/deployments/{deployment_id}': Endpoints.patch_Update_deployment_handler
    '/organisations/{organisation_id}/roles/{role_id}': Endpoints.patch_Update_role_handler
    '/releases/operator/{kind}/{version}': Endpoints.patch_Revise_release_handler
  }
}

// </EndpointByMethod>

// <EndpointByMethod.Shorthands>
export type GetEndpoints = EndpointByMethod['get']
export type PostEndpoints = EndpointByMethod['post']
export type PutEndpoints = EndpointByMethod['put']
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

  // <ApiClient.put>
  put<Path extends keyof PutEndpoints, TEndpoint extends PutEndpoints[Path]>(
    path: Path,
    ...params: MaybeOptionalArg<TEndpoint['parameters']>
  ): Promise<TEndpoint['response']> {
    return this.fetcher('put', this.baseUrl + path, params[0]).then((response) =>
      this.parseResponse(response)
    ) as Promise<TEndpoint['response']>
  }
  // </ApiClient.put>

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
