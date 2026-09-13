//! Reading what the cluster archived, through the resources the operator
//! keeps.
//!
//! Herald reads the Aether kind rather than CloudNativePG's own. Both exist on
//! these clusters, and only one of them says which deployment an archive
//! belonged to -- which is the single fact the control plane cannot look up
//! for itself.

use aether_crds::v1alpha::identity_instance_backup::{
    IdentityInstanceBackup, IdentityInstanceBackupStatus,
};
use kube::{Api, Client, ResourceExt, api::ListParams};
use tracing::warn;
use uuid::Uuid;

use crate::domain::{
    entities::{
        archive::{Archive, FailedArchive, TakenArchive},
        deployment::DeploymentId,
    },
    error::HeraldError,
    ports::ArchiveSource,
};

/// Reads archives through the Kubernetes API of the cluster Herald runs in.
pub struct KubeArchiveSource {
    client: Client,
}

impl KubeArchiveSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Builds a client from the ambient configuration: the service account
    /// when running as a pod, the local kubeconfig otherwise.
    pub async fn from_env() -> Result<Self, HeraldError> {
        let client = Client::try_default()
            .await
            .map_err(|error| HeraldError::Internal {
                message: format!("failed to build a Kubernetes client: {error}"),
            })?;

        Ok(Self::new(client))
    }
}

impl ArchiveSource for KubeArchiveSource {
    async fn finished_archives(&self) -> Result<Vec<Archive>, HeraldError> {
        let backups: Api<IdentityInstanceBackup> = Api::all(self.client.clone());

        let listed = backups
            .list(&ListParams::default())
            .await
            .map_err(|error| HeraldError::Internal {
                message: format!("the archives on this cluster could not be listed: {error}"),
            })?;

        Ok(listed
            .items
            .iter()
            .filter_map(|backup| {
                let name = backup.name_any();
                let archive = archive_from(
                    &backup.spec.identity_instance_ref.name,
                    backup.status.as_ref(),
                );

                if archive.is_none() {
                    // Not a failure. An archive that started a second ago has
                    // no size and no end, and reporting one would be inventing
                    // facts about a backup that does not exist yet.
                    warn!(archive = %name, "nothing to report about this archive yet");
                }

                archive
            })
            .collect())
    }
}

/// One archive, as the control plane needs to hear about it.
///
/// Pure, so what Herald would say can be asserted without a cluster. `None` is
/// an archive still in flight: every field describing a finished one has to be
/// there, because the control plane refuses a partial report rather than
/// filling it in -- guessing at a field is how an archive nobody can read gets
/// recorded as one that can.
fn archive_from(instance: &str, status: Option<&IdentityInstanceBackupStatus>) -> Option<Archive> {
    let deployment_id = deployment_of_instance(instance)?;
    let status = status?;

    if let Some(reason) = status.error.as_ref() {
        return Some(Archive::Failed(FailedArchive {
            deployment_id,
            reason: reason.clone(),
        }));
    }

    Some(Archive::Taken(TakenArchive {
        deployment_id,
        object_key: status.manifest_key.clone()?,
        postgres_major: status.postgres_major.as_ref()?.parse().ok()?,
        size_bytes: status.size_bytes.clone()?,
        started_at: status.started_at.as_ref()?.0,
        finished_at: status.stopped_at.as_ref()?.0,
    }))
}

/// The deployment an instance belongs to.
///
/// The name is `deployment-<uuid>`, written by the component that creates the
/// instance. Parsed rather than carried on the resource because it is already
/// the resource's name, and a second copy is a second thing that can be wrong.
fn deployment_of_instance(instance: &str) -> Option<DeploymentId> {
    let id = instance.strip_prefix("deployment-")?;

    Uuid::parse_str(id)
        .ok()
        .map(|id| DeploymentId::new(id.to_string()))
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;

    use super::*;

    const INSTANCE: &str = "deployment-b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d";

    fn finished() -> IdentityInstanceBackupStatus {
        IdentityInstanceBackupStatus {
            manifest_key: Some("aether/nightly-20260913.json".to_string()),
            postgres_major: Some("17".to_string()),
            size_bytes: Some("4096".to_string()),
            started_at: Some(Time(Utc.with_ymd_and_hms(2026, 9, 13, 2, 30, 0).unwrap())),
            stopped_at: Some(Time(Utc.with_ymd_and_hms(2026, 9, 13, 2, 34, 0).unwrap())),
            ..Default::default()
        }
    }

    #[test]
    fn a_finished_archive_says_everything_the_control_plane_requires() {
        let Some(Archive::Taken(taken)) = archive_from(INSTANCE, Some(&finished())) else {
            panic!("a finished archive was not reported");
        };

        assert_eq!(taken.object_key, "aether/nightly-20260913.json");
        assert_eq!(taken.postgres_major, 17);
        assert_eq!(taken.size_bytes, "4096");
        assert_eq!(
            taken.deployment_id.0,
            "b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d"
        );
    }

    /// The control plane refuses a partial report rather than filling it in,
    /// so an archive still being written is not sent at all. Reporting one
    /// would be a claim about a backup that does not exist yet.
    #[test]
    fn an_archive_still_being_written_is_not_reported() {
        for missing in [
            IdentityInstanceBackupStatus {
                size_bytes: None,
                ..finished()
            },
            IdentityInstanceBackupStatus {
                stopped_at: None,
                ..finished()
            },
            IdentityInstanceBackupStatus {
                manifest_key: None,
                ..finished()
            },
            IdentityInstanceBackupStatus {
                postgres_major: None,
                ..finished()
            },
        ] {
            assert!(archive_from(INSTANCE, Some(&missing)).is_none());
        }

        assert!(archive_from(INSTANCE, None).is_none());
    }

    /// An attempt that produced nothing is still worth saying. A backup that
    /// failed silently cannot be told from one that never ran.
    #[test]
    fn an_attempt_that_produced_nothing_is_reported_as_one() {
        let refused = IdentityInstanceBackupStatus {
            error: Some("the object store refused the upload".to_string()),
            ..Default::default()
        };

        let Some(Archive::Failed(failed)) = archive_from(INSTANCE, Some(&refused)) else {
            panic!("a failed attempt was not reported");
        };

        assert_eq!(failed.reason, "the object store refused the upload");
    }

    /// A failure is a failure even when half a status survived beside it.
    #[test]
    fn a_failure_beside_a_size_is_still_a_failure() {
        let both = IdentityInstanceBackupStatus {
            error: Some("interrupted".to_string()),
            ..finished()
        };

        assert!(matches!(
            archive_from(INSTANCE, Some(&both)),
            Some(Archive::Failed(_))
        ));
    }

    #[test]
    fn an_instance_this_platform_did_not_name_belongs_to_no_deployment() {
        assert!(deployment_of_instance("some-other-thing").is_none());
        assert!(deployment_of_instance("deployment-not-a-uuid").is_none());
        assert!(deployment_of_instance(INSTANCE).is_some());
    }
}
