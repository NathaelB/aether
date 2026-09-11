//! Getting from the version a deployment runs to the one it should.
//!
//! Not a jump. IAM products require stepping stones between some versions,
//! and targeting the final one works fine until the first customer three
//! releases behind, at which point it breaks their database. The path is what
//! the catalogue knows and the deployment does not.

use crate::{CoreError, catalog::Release, version::Version};

/// The versions to move through, in order, ending at the target.
///
/// Always at least one element. A path with nothing in it is not an upgrade
/// that needs no steps, it is a request that should have been refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradePath {
    steps: Vec<Version>,
}

impl UpgradePath {
    /// Works out the route from `from` to the target release.
    ///
    /// `catalogue` is every release of the product, in any order. The steps a
    /// release declares are its own: a release says what must be passed
    /// through to reach it, because that is knowledge its author has and the
    /// platform does not.
    pub fn plan(
        from: &Version,
        target: &Release,
        catalogue: &[Release],
    ) -> Result<Self, CoreError> {
        let mut steps: Vec<Version> = target
            .steps_through
            .iter()
            .filter(|step| *step > from && **step < target.id.version)
            .cloned()
            .collect();

        steps.sort();
        steps.dedup();
        steps.push(target.id.version.clone());

        // Checked after the route is known rather than while building it: a
        // path that runs through a version nobody may install is refused as a
        // whole, and saying which step is the problem is worth more than
        // silently routing around it.
        for step in &steps {
            let release = catalogue
                .iter()
                .find(|release| &release.id.version == step)
                .ok_or_else(|| CoreError::ReleaseNotFound {
                    release: format!("{} {step}", target.id.kind),
                })?;

            if !release.status.is_installable() {
                return Err(CoreError::ReleaseNotInstallable {
                    release: release.id.to_string(),
                    status: format!("{:?}", release.status).to_lowercase(),
                });
            }
        }

        Ok(Self { steps })
    }

    /// A path of one step, for a target that declares none.
    pub fn direct(target: Version) -> Self {
        Self {
            steps: vec![target],
        }
    }

    pub fn steps(&self) -> &[Version] {
        &self.steps
    }

    /// The next version to apply, given what is running now.
    ///
    /// Driven by what the deployment reports rather than by a counter the
    /// control plane keeps. A counter and a cluster disagree the moment one of
    /// them restarts, and the cluster is the one that is right.
    pub fn next_after(&self, current: &Version) -> Option<&Version> {
        self.steps.iter().find(|step| *step > current)
    }

    pub fn target(&self) -> &Version {
        self.steps.last().expect("a path always has a last step")
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        catalog::{BreakingRisk, ReleaseId, ReleaseNotes, ReleaseStatus},
        deployments::DeploymentKind,
    };
    use chrono::Utc;

    fn release(version: Version, status: ReleaseStatus, steps: Vec<Version>) -> Release {
        let mut release = Release::announce(
            ReleaseId::new(DeploymentKind::Ferriskey, version),
            BreakingRisk::None,
            ReleaseNotes("notes".to_string()),
            Utc::now(),
        );
        release.status = status;
        release.steps_through = steps;
        release
    }

    fn v(major: u64, minor: u64, patch: u64) -> Version {
        Version::new(major, minor, patch)
    }

    fn available(version: Version) -> Release {
        release(version, ReleaseStatus::Available, Vec::new())
    }

    #[test]
    fn a_target_with_no_steps_is_one_hop() {
        let target = available(v(26, 0, 1));
        let catalogue = vec![target.clone()];

        let path = UpgradePath::plan(&v(26, 0, 0), &target, &catalogue).expect("planned");

        assert_eq!(path.steps(), [v(26, 0, 1)]);
        assert_eq!(path.target(), &v(26, 0, 1));
    }

    /// The reason this exists. A customer three releases behind has to pass
    /// through the migrations in between, and the target is what knows which.
    #[test]
    fn a_target_that_declares_steps_produces_them_in_order() {
        let target = release(
            v(27, 0, 0),
            ReleaseStatus::Available,
            vec![v(26, 0, 0), v(25, 0, 0)],
        );
        let catalogue = vec![
            available(v(25, 0, 0)),
            available(v(26, 0, 0)),
            target.clone(),
        ];

        let path = UpgradePath::plan(&v(24, 0, 0), &target, &catalogue).expect("planned");

        assert_eq!(path.steps(), [v(25, 0, 0), v(26, 0, 0), v(27, 0, 0)]);
    }

    /// A deployment already past a stepping stone does not go back to it.
    #[test]
    fn a_step_already_behind_the_deployment_is_skipped() {
        let target = release(
            v(27, 0, 0),
            ReleaseStatus::Available,
            vec![v(25, 0, 0), v(26, 0, 0)],
        );
        let catalogue = vec![
            available(v(25, 0, 0)),
            available(v(26, 0, 0)),
            target.clone(),
        ];

        let path = UpgradePath::plan(&v(25, 5, 0), &target, &catalogue).expect("planned");

        assert_eq!(path.steps(), [v(26, 0, 0), v(27, 0, 0)]);
    }

    /// Routing around a withdrawn version would mean skipping the migration
    /// it carried. Refusing says which one is the problem.
    #[test]
    fn a_path_through_a_withdrawn_version_is_refused() {
        let target = release(v(27, 0, 0), ReleaseStatus::Available, vec![v(26, 0, 0)]);
        let catalogue = vec![
            release(v(26, 0, 0), ReleaseStatus::Withdrawn, Vec::new()),
            target.clone(),
        ];

        let error = UpgradePath::plan(&v(25, 0, 0), &target, &catalogue)
            .expect_err("a step that must not be installed");

        assert!(matches!(error, CoreError::ReleaseNotInstallable { .. }));
        assert!(error.to_string().contains("26.0.0"), "{error}");
    }

    /// A deprecated stepping stone is fine. It still runs and can still be
    /// moved through, which is exactly what a stepping stone is for.
    #[test]
    fn a_deprecated_step_is_allowed() {
        let target = release(v(27, 0, 0), ReleaseStatus::Available, vec![v(26, 0, 0)]);
        let catalogue = vec![
            release(v(26, 0, 0), ReleaseStatus::Deprecated, Vec::new()),
            target.clone(),
        ];

        let path = UpgradePath::plan(&v(25, 0, 0), &target, &catalogue).expect("planned");

        assert_eq!(path.steps(), [v(26, 0, 0), v(27, 0, 0)]);
    }

    #[test]
    fn a_step_the_catalogue_does_not_hold_is_refused() {
        let target = release(v(27, 0, 0), ReleaseStatus::Available, vec![v(26, 0, 0)]);
        let catalogue = vec![target.clone()];

        let error = UpgradePath::plan(&v(25, 0, 0), &target, &catalogue)
            .expect_err("a step nobody published");

        assert!(matches!(error, CoreError::ReleaseNotFound { .. }));
    }

    /// The next step is read from what the deployment reports, not from a
    /// counter. A counter and a cluster disagree the moment one restarts, and
    /// the cluster is the one that is right.
    #[test]
    fn the_next_step_is_the_first_one_ahead_of_what_runs() {
        let path = UpgradePath {
            steps: vec![v(25, 0, 0), v(26, 0, 0), v(27, 0, 0)],
        };

        assert_eq!(path.next_after(&v(24, 0, 0)), Some(&v(25, 0, 0)));
        assert_eq!(path.next_after(&v(25, 0, 0)), Some(&v(26, 0, 0)));
        assert_eq!(path.next_after(&v(26, 0, 0)), Some(&v(27, 0, 0)));
    }

    #[test]
    fn a_deployment_on_the_target_has_nothing_left() {
        let path = UpgradePath {
            steps: vec![v(26, 0, 0), v(27, 0, 0)],
        };

        assert_eq!(path.next_after(&v(27, 0, 0)), None);
    }

    /// Declared out of order, or twice. The catalogue is edited by people.
    #[test]
    fn steps_are_ordered_and_deduplicated_however_they_were_written() {
        let target = release(
            v(27, 0, 0),
            ReleaseStatus::Available,
            vec![v(26, 0, 0), v(25, 0, 0), v(26, 0, 0)],
        );
        let catalogue = vec![
            available(v(25, 0, 0)),
            available(v(26, 0, 0)),
            target.clone(),
        ];

        let path = UpgradePath::plan(&v(24, 0, 0), &target, &catalogue).expect("planned");

        assert_eq!(path.steps(), [v(25, 0, 0), v(26, 0, 0), v(27, 0, 0)]);
    }

    /// A step at or beyond the target is not a step towards it. Included, it
    /// would either repeat the target or overshoot it.
    #[test]
    fn a_step_at_or_past_the_target_is_dropped() {
        let target = release(
            v(27, 0, 0),
            ReleaseStatus::Available,
            vec![v(27, 0, 0), v(28, 0, 0), v(26, 0, 0)],
        );
        let catalogue = vec![available(v(26, 0, 0)), target.clone()];

        let path = UpgradePath::plan(&v(25, 0, 0), &target, &catalogue).expect("planned");

        assert_eq!(path.steps(), [v(26, 0, 0), v(27, 0, 0)]);
    }
}
