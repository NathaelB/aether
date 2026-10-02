use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use aether_crds::v1alpha::identity_dataplane_upgrade::{
    ComponentUpgradeState, ComponentUpgradeStatus, DataplaneComponent, DataplaneUpgradePhase,
    IdentityDataplaneUpgrade, IdentityDataplaneUpgradeSpec, IdentityDataplaneUpgradeStatus,
};
use aether_operator_core::domain::dataplane_upgrade::service::{
    Observed, Step, advance, decide, preflight,
};
use aether_operator_core::domain::dataplane_upgrade::{ComponentVersions, DataplaneComponentKind};
use chrono::{DateTime, Duration as Delta, SecondsFormat, Utc};
use genesis_core::domain::entities::dataplane_upgrade::DesiredDataplaneUpgrade;
use genesis_core::domain::ports::EventHandler;
use serde_json::Value;

use super::upgrades::{InMemoryUpgrades, dataplane, genesis_event, handler};

const TIMEOUT: Duration = Duration::from_secs(300);
const TICK: i64 = 10;

#[derive(Debug, Clone)]
pub struct Cluster {
    pub versions: ComponentVersions,
    pub ready: BTreeMap<DataplaneComponentKind, bool>,
    pub never_ready: Option<DataplaneComponentKind>,
    pub patched: Vec<DataplaneComponentKind>,
    pub restored: Vec<(DataplaneComponentKind, String)>,
}

impl Cluster {
    pub fn at(version: &str) -> Self {
        Self {
            versions: DataplaneComponentKind::ALL
                .into_iter()
                .map(|kind| (kind, version.to_string()))
                .collect(),
            ready: DataplaneComponentKind::ALL
                .into_iter()
                .map(|kind| (kind, true))
                .collect(),
            never_ready: None,
            patched: Vec::new(),
            restored: Vec::new(),
        }
    }

    pub fn observed(&self, now: DateTime<Utc>) -> Observed {
        Observed {
            versions: self.versions.clone(),
            ready: self.ready.clone(),
            now,
        }
    }

    pub fn apply(&mut self, step: &Step, target: &str) {
        match step {
            Step::Patch(kind) => {
                self.patched.push(*kind);
                self.versions.insert(*kind, target.to_string());
                self.ready.insert(*kind, self.never_ready != Some(*kind));
            }
            Step::RollBack { restore, .. } => {
                for (kind, version) in restore {
                    self.versions.insert(*kind, version.clone());
                    self.ready.insert(*kind, true);
                    self.restored.push((*kind, version.clone()));
                }
            }
            _ => {}
        }
    }
}

pub fn drive(
    upgrade: &IdentityDataplaneUpgrade,
    cluster: &mut Cluster,
) -> IdentityDataplaneUpgradeStatus {
    let status = upgrade.status.clone().unwrap_or_default();
    assert_eq!(status.phase, DataplaneUpgradePhase::Pending);
    drive_from(&upgrade.spec, status, cluster)
}

pub fn drive_from(
    spec: &IdentityDataplaneUpgradeSpec,
    mut status: IdentityDataplaneUpgradeStatus,
    cluster: &mut Cluster,
) -> IdentityDataplaneUpgradeStatus {
    let start = Utc::now();

    for tick in 0..200 {
        let now = start + Delta::seconds(tick * TICK);
        let step = preflight(spec, &status)
            .unwrap_or_else(|| decide(spec, &status, &cluster.observed(now), TIMEOUT));
        status = advance(spec, &status, &step, now);
        cluster.apply(&step, &spec.target_version);
        if matches!(step, Step::Idle) {
            return status;
        }
    }
    panic!("the upgrade never settled, last status: {status:?}");
}

pub fn halfway(
    cluster: &mut Cluster,
    target: &str,
    previous: &str,
) -> IdentityDataplaneUpgradeStatus {
    cluster
        .versions
        .insert(DataplaneComponentKind::Herald, target.to_string());
    cluster
        .versions
        .insert(DataplaneComponentKind::Genesis, target.to_string());
    let started_at = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
    let recorded = |name, state| ComponentUpgradeStatus {
        name,
        previous_version: Some(previous.to_string()),
        state,
        started_at: started_at.clone(),
    };
    IdentityDataplaneUpgradeStatus {
        phase: DataplaneUpgradePhase::Upgrading,
        components: vec![
            recorded(DataplaneComponent::Herald, ComponentUpgradeState::Upgraded),
            recorded(
                DataplaneComponent::Genesis,
                ComponentUpgradeState::Upgrading,
            ),
        ],
        ..Default::default()
    }
}

pub async fn desired_after_the_handler(
    wire: Value,
) -> (Arc<InMemoryUpgrades>, DesiredDataplaneUpgrade) {
    let upgrades = Arc::new(InMemoryUpgrades::default());
    handler(&upgrades)
        .handle(genesis_event(wire))
        .await
        .expect("the handler accepts the event");
    let desired = upgrades.desired(&dataplane().to_string());
    (upgrades, desired)
}
