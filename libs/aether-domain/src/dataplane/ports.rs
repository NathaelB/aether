use std::future::Future;

use aether_auth::Identity;
use chrono::{DateTime, Utc};

use crate::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        herald_identity::{MintedHeraldIdentity, RegisteredDataPlane},
        value_objects::{
            CreateDataplaneCommand, DataPlaneId, DataPlaneMode, DeploymentResources,
            ListDataPlaneDeploymentsCommand, PlacementRequest, Region, ServiceIntent,
        },
    },
    deployments::{Deployment, commands::ReportDeploymentOutcomeCommand},
    organisation::OrganisationId,
    version::Version,
};

pub trait DataPlaneService: Send + Sync {
    /// Registers a data plane and mints the identity its Herald will use.
    ///
    /// Answers with the secret, once. It is never stored, so this is the only
    /// moment it can be read -- and re-issuing is how an installation that
    /// lost it, or leaked it, gets another.
    fn create_dataplane(
        &self,
        identity: Identity,
        command: CreateDataplaneCommand,
    ) -> impl Future<Output = Result<RegisteredDataPlane, CoreError>> + Send;

    /// Replaces the credential a data plane's Herald authenticates with.
    ///
    /// The old one stops working at once. That is the point: a secret nobody
    /// can invalidate is a secret that has to be assumed still in somebody's
    /// hands.
    fn reissue_herald_credential(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> impl Future<Output = Result<RegisteredDataPlane, CoreError>> + Send;
    /// Takes a data plane out of service, or puts it back.
    ///
    /// Changing what the fleet will accept, not reading it: the same right
    /// that registers a cluster is the one that stops work going to it.
    fn set_dataplane_service(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
        service: ServiceIntent,
    ) -> impl Future<Output = Result<DataPlane, CoreError>> + Send;

    fn list_dataplanes(
        &self,
        identity: Identity,
    ) -> impl Future<Output = Result<Vec<DataPlane>, CoreError>> + Send;
    fn get_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> impl Future<Output = Result<DataPlane, CoreError>> + Send;
    fn get_deployments_in_dataplane(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
        command: ListDataPlaneDeploymentsCommand,
    ) -> impl Future<Output = Result<Vec<Deployment>, CoreError>> + Send;

    /// Records that a data plane's Herald is alive.
    ///
    /// Returns `false` for an unknown id rather than failing: a Herald
    /// configured with a stale id should be told so, not served a 500 that
    /// reads like the control plane is broken.
    /// Records what a data plane observed happening to one of its deployments.
    ///
    /// The only path by which the control plane learns anything about a
    /// deployment after handing it over: everything downstream of that hand-off
    /// happens inside a cluster the control plane cannot see into.
    ///
    /// Returns `false` when the report changed nothing -- an unknown
    /// deployment, or one whose state does not accept this outcome. Not an
    /// error: reports are at-least-once, and answering a redelivered one with a
    /// failure would have a data plane retry something already recorded.
    /// The regions a deployment can be created in.
    ///
    /// Deliberately not derived from the data plane inventory by the caller:
    /// choosing where to run is a customer's decision, while how many clusters
    /// serve a region and who owns them is not their business.
    fn list_regions(
        &self,
        identity: Identity,
    ) -> impl Future<Output = Result<Vec<Region>, CoreError>> + Send;

    /// All data planes, for background system probes.
    ///
    /// Takes no `Identity` for the same reason background jobs like
    /// `purge_deleted_deployments` do not: nobody is the caller, the
    /// installation's own upkeep is. This is the unchecked read; callers who
    /// need authorization (like an API endpoint listing for a user) should use
    /// `list_dataplanes(identity)` instead.
    fn list_all_dataplanes(&self)
    -> impl Future<Output = Result<Vec<DataPlane>, CoreError>> + Send;

    /// Takes proof of which data plane is speaking, not an identity to test.
    ///
    /// The proof can only be obtained by reading it from a credential, so a
    /// caller cannot name the data plane it is reporting for.
    fn report_outcome(
        &self,
        identity: Identity,
        command: ReportDeploymentOutcomeCommand,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;

    /// `operator_version` and `gateway_address` are each `None` when the
    /// Herald sending the heartbeat does not report one; the stored value is
    /// left untouched rather than cleared, since a missing report is not
    /// evidence the fact changed.
    fn record_heartbeat(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
        operator_version: Option<Version>,
        gateway_address: Option<String>,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;
}

/// Minting the identity a data plane authenticates with.
///
/// A port because it is an identity provider's job, and this platform should
/// not care which one: creating a client is the one administrative act the
/// control plane performs on the realm, and it is worth being able to see it
/// in one place and swap it in another.
///
/// Deliberately narrow. The credentials behind this create and delete clients
/// in a realm, which is a privilege no request-handling code should be able to
/// reach for anything else.
pub trait HeraldIdentityProvisioner: Send + Sync {
    /// Creates a client for this data plane and returns its secret, once.
    ///
    /// Called again for the same data plane, it replaces the credential: that
    /// is what rotating one is, and an installation that could not rotate
    /// would keep a leaked secret until somebody rebuilt the cluster.
    fn mint(
        &self,
        dataplane: DataPlaneId,
    ) -> impl Future<Output = Result<MintedHeraldIdentity, CoreError>> + Send;

    /// Removes the client, so a retired cluster cannot authenticate.
    fn revoke(&self, dataplane: DataPlaneId) -> impl Future<Output = Result<(), CoreError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait DataPlaneRepository: Send + Sync {
    /// The data plane a caller is allowed to speak for.
    ///
    /// Resolved from the subject its token carries, so which data plane is
    /// acting is never something the caller says. `None` means the caller is
    /// not a Herald this installation knows.
    fn find_by_herald_subject(
        &self,
        subject: &str,
    ) -> impl Future<Output = Result<Option<DataPlane>, CoreError>> + Send;

    fn find_by_id(
        &self,
        id: &DataPlaneId,
    ) -> impl Future<Output = Result<Option<DataPlane>, CoreError>> + Send;
    fn find_active_shared_by_region(
        &self,
        region: &Region,
    ) -> impl Future<Output = Result<Vec<DataPlane>, CoreError>> + Send;
    /// Finds a data plane that can host this deployment.
    ///
    /// Excludes anything not `Active`, anything that has not reported since
    /// `seen_since`, anything without room in all three dimensions, and any
    /// dedicated data plane belonging to another organisation.
    fn find_available(
        &self,
        request: PlacementRequest,
    ) -> impl Future<Output = Result<Option<DataPlane>, CoreError>> + Send;

    /// Whether any data plane at all exists in this region, whatever its mode,
    /// status or load.
    ///
    /// Asked only once placement has already failed, to tell "come back later"
    /// apart from "this installation does not serve that region" -- two answers
    /// a caller acts on differently.
    fn region_is_served(
        &self,
        region: &Region,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;

    /// Whether a plane in this region would have had room for `resources` if
    /// its optional deployment-count bound had not applied.
    ///
    /// Asked only once `find_available` has already failed and the region is
    /// served -- the same once-a-failure computation `region_is_served`
    /// already does -- so a caller can tell "every dimension is exhausted"
    /// apart from "resources have room, the count does not".
    fn region_blocked_by_deployment_count(
        &self,
        region: &Region,
        mode: DataPlaneMode,
        resources: DeploymentResources,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;

    /// The data plane belonging to this organisation in this region, whatever
    /// its status or liveness.
    ///
    /// Deliberately unfiltered: this is asked *before* provisioning, to find a
    /// cluster that exists but has not reported yet. Filtering it the way
    /// `find_available` does would hide exactly the case it exists to catch
    /// and provision a second cluster for an organisation that already has
    /// one.
    fn find_dedicated_for_organisation(
        &self,
        organisation_id: &OrganisationId,
        region: &Region,
    ) -> impl Future<Output = Result<Option<DataPlane>, CoreError>> + Send;
    fn list_all(&self) -> impl Future<Output = Result<Vec<DataPlane>, CoreError>> + Send;
    fn current_load(&self, id: &DataPlaneId)
    -> impl Future<Output = Result<u32, CoreError>> + Send;
    fn save(&self, dataplane: &DataPlane) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Stamps `last_seen_at`, and promotes a `Provisioning` data plane to
    /// `Active`.
    ///
    /// The promotion belongs here because a heartbeat is the only evidence the
    /// control plane ever gets that a cluster finished coming up: it means
    /// Herald is running inside it and talking. Nothing else could make the
    /// transition -- registering a data plane says it should exist, and the
    /// control plane cannot reach into a cluster to ask.
    ///
    /// Also records the operator/chart version and the Gateway address this
    /// heartbeat carries, when it carries them. Left untouched when it does
    /// not: an installation whose Herald has not been updated to send one, or
    /// a cycle where the value briefly could not be read, is not evidence
    /// either one changed.
    ///
    /// Returns `false` when no such data plane exists.
    fn touch_last_seen(
        &self,
        id: &DataPlaneId,
        at: DateTime<Utc>,
        operator_version: Option<Version>,
        gateway_address: Option<String>,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;
}
