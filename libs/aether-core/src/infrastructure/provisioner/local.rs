use tracing::{info, warn};

use crate::domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        provisioner::{ClusterProvisioner, ProvisionRequest},
        value_objects::DataPlaneId,
    },
};

/// Refuses to provision, and says why.
///
/// This started out registering a `DataPlane` record pointing at the k3d
/// cluster `make local-up` already creates, on the reasoning that the cluster
/// exists so only the record is missing. That reasoning was wrong, and it was
/// wrong in the worst way: the record was created, the deployment was placed on
/// it, and nothing ever served either. No Herald claims for that id -- the one
/// running in the local cluster is configured with a different data plane -- so
/// the deployment waited in `Pending` for ever, beside a data plane stuck in
/// `Provisioning`, with nothing anywhere saying what had gone wrong.
///
/// `dedicated` means a cluster of the organisation's own (#36). An adapter that
/// creates no cluster cannot satisfy that, and pretending otherwise produces a
/// system that looks like it is working. Failing immediately, with a message
/// naming both ways forward, is the honest implementation of "there is no
/// provisioner here".
///
/// #43 is the adapter that makes this work for real.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalClusterProvisioner;

impl ClusterProvisioner for LocalClusterProvisioner {
    async fn provision(&self, request: ProvisionRequest) -> Result<DataPlane, CoreError> {
        warn!(
            organisation_id = %request.organisation_id.0,
            region = %request.region.as_str(),
            "refusing to provision: no cluster provisioner is configured"
        );

        Err(CoreError::ProvisioningUnavailable {
            reason: format!(
                "This installation cannot create clusters, so it cannot serve a dedicated \
                 deployment in '{}'. Either create it in shared mode, or register a data \
                 plane for this organisation and install the aether-dataplane chart into \
                 its cluster -- the next dedicated deployment will be placed on it.",
                request.region.as_str()
            ),
        })
    }

    /// Succeeds, and does nothing.
    ///
    /// Nothing here ever created infrastructure, so there is none to destroy.
    /// Returning an error instead would break cleanup paths that call this
    /// unconditionally, over a data plane record whose removal is the
    /// repository's job.
    async fn deprovision(&self, id: &DataPlaneId) -> Result<(), CoreError> {
        info!(dataplane_id = %id, "local provisioner has nothing to deprovision");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        dataplane::value_objects::{DeploymentResources, Region},
        organisation::OrganisationId,
    };
    use uuid::Uuid;

    fn request() -> ProvisionRequest {
        ProvisionRequest {
            organisation_id: OrganisationId(Uuid::new_v4()),
            region: Region::new("local"),
            minimum: DeploymentResources::DEFAULT,
        }
    }

    /// The regression this replaced. Returning a data plane here meant the
    /// caller saved it, placed a deployment on it, and left both waiting on a
    /// Herald that would never exist.
    #[tokio::test]
    async fn provisioning_fails_rather_than_registering_a_data_plane_nothing_serves() {
        let error = LocalClusterProvisioner
            .provision(request())
            .await
            .expect_err("there is no cluster provisioner here");

        assert!(matches!(error, CoreError::ProvisioningUnavailable { .. }));
    }

    /// The message is the whole value of failing here rather than silently
    /// succeeding, so it is worth asserting that it stays actionable.
    #[tokio::test]
    async fn the_refusal_names_both_ways_forward() {
        let error = LocalClusterProvisioner
            .provision(request())
            .await
            .expect_err("there is no cluster provisioner here");
        let message = error.to_string();

        assert!(message.contains("shared"), "{message}");
        assert!(message.contains("register a data plane"), "{message}");
        assert!(message.contains("local"), "the region is named: {message}");
    }

    /// Cleanup paths call this unconditionally. Failing because nothing was
    /// ever created would break them over a record the repository removes.
    #[tokio::test]
    async fn deprovisioning_always_succeeds() {
        let result = LocalClusterProvisioner
            .deprovision(&DataPlaneId(Uuid::new_v4()))
            .await;

        assert!(result.is_ok());
    }
}
