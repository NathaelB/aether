use aether_auth::Identity;
use chrono::{DateTime, Utc};

use crate::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        value_objects::{
            CreateDataplaneCommand, DataPlaneId, ListDataPlaneDeploymentsCommand, PlacementRequest,
            Region,
        },
    },
    deployments::Deployment,
    organisation::OrganisationId,
};

pub trait DataPlaneService: Send + Sync {
    fn create_dataplane(
        &self,
        identity: Identity,
        command: CreateDataplaneCommand,
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
    fn record_heartbeat(
        &self,
        identity: Identity,
        dataplane_id: DataPlaneId,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait DataPlaneRepository: Send + Sync {
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
    /// Returns `false` when no such data plane exists.
    fn touch_last_seen(
        &self,
        id: &DataPlaneId,
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;
}
