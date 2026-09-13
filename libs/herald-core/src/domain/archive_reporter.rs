//! Carrying what the cluster archived up to the control plane.
//!
//! Its own type rather than another method on [`HeraldService`]: reporting
//! archives needs one source and one destination, and folding it into a
//! service that already carries five collaborators would make every caller of
//! that service learn about a sixth it does not use.
//!
//! [`HeraldService`]: crate::domain::ports::HeraldService

use std::sync::Arc;

use tracing::{info, warn};

use crate::domain::{
    entities::dataplane::DataPlaneId,
    error::HeraldError,
    ports::{ArchiveSource, ControlPlaneRepository},
};

pub struct ArchiveReporter<CP, AS>
where
    CP: ControlPlaneRepository,
    AS: ArchiveSource,
{
    control_plane: Arc<CP>,
    archives: Arc<AS>,
    dataplane_id: DataPlaneId,
}

/// What one pass carried, and what it could not.
///
/// Returned rather than only logged so a test can state the rule that matters:
/// one archive nobody could report does not stop the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Reported {
    pub carried: usize,
    pub refused: usize,
}

impl<CP, AS> ArchiveReporter<CP, AS>
where
    CP: ControlPlaneRepository,
    AS: ArchiveSource,
{
    pub fn new(control_plane: Arc<CP>, archives: Arc<AS>, dataplane_id: DataPlaneId) -> Self {
        Self {
            control_plane,
            archives,
            dataplane_id,
        }
    }

    /// Says every finished archive upward, again.
    ///
    /// Everything the cluster still knows about is sent on every pass, and
    /// nothing is remembered between them. The control plane records an
    /// archive once per object key, so repeating is free -- and the state this
    /// would otherwise keep is state a restart loses, which for backups means
    /// an archive that exists and that nobody ever hears about.
    pub async fn report(&self) -> Result<Reported, HeraldError> {
        let archives = self.archives.finished_archives().await?;
        let mut reported = Reported::default();

        for archive in &archives {
            // One at a time, and one failure does not end the pass. The
            // archives after it are unrelated, and a single unreachable
            // deployment would otherwise hide every other backup this data
            // plane took.
            match self
                .control_plane
                .report_archive(&self.dataplane_id, archive)
                .await
            {
                Ok(()) => reported.carried += 1,
                Err(error) => {
                    reported.refused += 1;
                    warn!(
                        deployment = %archive.deployment_id(),
                        %error,
                        "an archive could not be reported: it will be sent again next pass"
                    );
                }
            }
        }

        if reported.carried > 0 {
            info!(
                carried = reported.carried,
                refused = reported.refused,
                "archives reported"
            );
        }

        Ok(reported)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::Utc;

    use super::*;
    use crate::domain::entities::{
        archive::{Archive, FailedArchive, TakenArchive},
        deployment::DeploymentId,
    };
    use crate::domain::ports::MockControlPlaneRepository;

    fn deployment(n: u128) -> DeploymentId {
        DeploymentId::new(uuid::Uuid::from_u128(n).to_string())
    }

    fn taken(n: u128) -> Archive {
        Archive::Taken(TakenArchive {
            deployment_id: deployment(n),
            object_key: format!("aether/nightly-{n}.json"),
            postgres_major: 17,
            size_bytes: "4096".to_string(),
            started_at: Utc::now(),
            finished_at: Utc::now(),
        })
    }

    struct Cluster(Vec<Archive>);

    impl ArchiveSource for Cluster {
        async fn finished_archives(&self) -> Result<Vec<Archive>, HeraldError> {
            Ok(self.0.clone())
        }
    }

    fn reporter(
        control_plane: MockControlPlaneRepository,
        archives: Vec<Archive>,
    ) -> ArchiveReporter<MockControlPlaneRepository, Cluster> {
        ArchiveReporter::new(
            Arc::new(control_plane),
            Arc::new(Cluster(archives)),
            DataPlaneId::new(uuid::Uuid::from_u128(1).to_string()),
        )
    }

    #[tokio::test]
    async fn every_archive_the_cluster_knows_about_is_carried() {
        let mut control_plane = MockControlPlaneRepository::new();
        control_plane
            .expect_report_archive()
            .times(2)
            .returning(|_, _| Box::pin(async { Ok(()) }));

        let reported = reporter(control_plane, vec![taken(1), taken(2)])
            .report()
            .await
            .expect("the cluster answered");

        assert_eq!(reported.carried, 2);
    }

    /// The rule this type exists for. A single unreachable deployment would
    /// otherwise hide every other backup this data plane took.
    #[tokio::test]
    async fn one_archive_nobody_could_report_does_not_stop_the_others() {
        let seen = Arc::new(Mutex::new(0));
        let counter = seen.clone();

        let mut control_plane = MockControlPlaneRepository::new();
        control_plane
            .expect_report_archive()
            .returning(move |_, _| {
                let mut count = counter.lock().expect("not poisoned");
                *count += 1;
                let first = *count == 1;

                Box::pin(async move {
                    if first {
                        Err(HeraldError::ControlPlane {
                            message: "no".to_string(),
                        })
                    } else {
                        Ok(())
                    }
                })
            });

        let reported = reporter(control_plane, vec![taken(1), taken(2), taken(3)])
            .report()
            .await
            .expect("the cluster answered");

        assert_eq!(reported.carried, 2);
        assert_eq!(reported.refused, 1);
    }

    /// An attempt that produced nothing is reported too. A backup that failed
    /// silently is indistinguishable from one that never ran.
    #[tokio::test]
    async fn an_attempt_that_produced_no_archive_is_still_reported() {
        let mut control_plane = MockControlPlaneRepository::new();
        control_plane
            .expect_report_archive()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));

        let failed = Archive::Failed(FailedArchive {
            deployment_id: deployment(1),
            reason: "the object store refused the upload".to_string(),
        });

        assert_eq!(
            reporter(control_plane, vec![failed])
                .report()
                .await
                .expect("the cluster answered")
                .carried,
            1
        );
    }

    #[tokio::test]
    async fn a_cluster_with_no_archives_says_nothing_upward() {
        let mut control_plane = MockControlPlaneRepository::new();
        control_plane.expect_report_archive().never();

        assert_eq!(
            reporter(control_plane, Vec::new())
                .report()
                .await
                .expect("the cluster answered"),
            Reported::default()
        );
    }
}
