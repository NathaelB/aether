use std::sync::Arc;

use aether_core::dataplane_upgrade::{
    DataplaneUpgradePayload, UpgradeComponent as ApiComponent, UpgradeStrategy as ApiStrategy,
};
use aether_crds::v1alpha::identity_dataplane_upgrade::DataplaneUpgradePhase;
use aether_domain::dataplane::value_objects::DataPlaneId as ApiDataPlaneId;
use aether_e2e::support::cluster::{Cluster, desired_after_the_handler, drive};
use aether_e2e::support::upgrades::{
    InMemoryUpgrades, NAMESPACE, PREVIOUS, TARGET, api_payload, dataplane, genesis_event, handler,
    herald_action, upgrade_wire, wire,
};
use aether_operator_core::domain::dataplane_upgrade::DataplaneComponentKind;
use genesis_core::domain::entities::dataplane_upgrade_payload::DataplaneUpgradePayloadV1;
use genesis_core::domain::ports::EventHandler;
use genesis_core::infrastructure::kubernetes::dataplane_upgrade::resource;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn an_upgrade_flows_from_the_api_payload_to_a_completed_status() {
    let (_, desired) = desired_after_the_handler(upgrade_wire(TARGET)).await;
    assert_eq!(desired.name, dataplane().to_string());
    assert_eq!(desired.namespace, NAMESPACE);

    let upgrade = resource(&desired);
    assert_eq!(upgrade.spec.target_version, TARGET);
    let mut cluster = Cluster::at(PREVIOUS);

    let status = drive(&upgrade, &mut cluster);

    assert_eq!(status.phase, DataplaneUpgradePhase::Completed);
    assert_eq!(status.current_version.as_deref(), Some(TARGET));
    assert_eq!(status.progress.as_deref(), Some("3/3 components updated"));
    assert_eq!(
        cluster.patched,
        vec![
            DataplaneComponentKind::Herald,
            DataplaneComponentKind::Genesis,
            DataplaneComponentKind::Operator,
        ]
    );
    assert!(cluster.versions.values().all(|version| version == TARGET));
}

#[tokio::test]
async fn a_component_that_never_becomes_ready_rolls_the_others_back_in_reverse_order() {
    let (_, desired) = desired_after_the_handler(upgrade_wire(TARGET)).await;
    let upgrade = resource(&desired);
    let mut cluster = Cluster::at(PREVIOUS);
    cluster.never_ready = Some(DataplaneComponentKind::Genesis);

    let status = drive(&upgrade, &mut cluster);

    assert_eq!(status.phase, DataplaneUpgradePhase::RolledBack);
    assert_eq!(status.current_version, None);
    assert_eq!(
        cluster.patched,
        vec![
            DataplaneComponentKind::Herald,
            DataplaneComponentKind::Genesis
        ]
    );
    assert_eq!(
        cluster.restored,
        vec![
            (DataplaneComponentKind::Genesis, PREVIOUS.to_string()),
            (DataplaneComponentKind::Herald, PREVIOUS.to_string()),
        ]
    );
    assert!(cluster.versions.values().all(|version| version == PREVIOUS));
}

#[test]
fn the_envelope_of_a_data_plane_action_has_no_deployment_id_key() {
    let value = upgrade_wire(TARGET);

    let keys = value.as_object().expect("an object");
    assert!(!keys.contains_key("deployment_id"), "{value}");
    assert_eq!(value["routing_key"], "dataplane.upgrade");
    assert_eq!(genesis_event(value).deployment_id, None);
}

#[test]
fn the_envelope_of_a_deployment_action_still_carries_its_deployment_id() {
    let deployment = Uuid::from_u128(9);
    let value = wire(herald_action(
        Some(deployment),
        "deployment.create",
        json!({}),
    ));

    assert_eq!(value["deployment_id"], deployment.to_string());
    assert_eq!(genesis_event(value).deployment_id, Some(deployment));
}

#[test]
fn a_payload_without_components_is_rejected_by_the_api_and_by_genesis() {
    assert!(
        DataplaneUpgradePayload::new(
            ApiDataPlaneId(dataplane()),
            TARGET.to_string(),
            vec![],
            ApiStrategy::Rolling,
            1,
        )
        .is_err()
    );

    let mut payload = api_payload(TARGET, vec![ApiComponent::All], 1);
    payload["components"] = json!([]);

    assert!(DataplaneUpgradePayloadV1::from_value(&payload).is_err());
}

#[test]
fn a_max_unavailable_of_zero_is_rejected_by_the_api_and_by_genesis() {
    assert!(
        DataplaneUpgradePayload::new(
            ApiDataPlaneId(dataplane()),
            TARGET.to_string(),
            vec![ApiComponent::All],
            ApiStrategy::Rolling,
            0,
        )
        .is_err()
    );

    let mut payload = api_payload(TARGET, vec![ApiComponent::All], 1);
    payload["max_unavailable"] = json!(0);

    assert!(DataplaneUpgradePayloadV1::from_value(&payload).is_err());
}

#[test]
fn the_api_payload_is_accepted_by_genesis() {
    let payload = api_payload(TARGET, vec![ApiComponent::All], 3);

    let parsed = DataplaneUpgradePayloadV1::from_value(&payload).expect("a valid payload");

    assert_eq!(parsed.dataplane_id, dataplane());
    assert_eq!(parsed.target_version, TARGET);
    assert_eq!(parsed.max_unavailable, 3);
}

#[tokio::test]
async fn a_duplicate_event_creates_one_upgrade() {
    let upgrades = Arc::new(InMemoryUpgrades::default());
    let handler = handler(&upgrades);

    handler
        .handle(genesis_event(upgrade_wire(TARGET)))
        .await
        .expect("created");
    handler
        .handle(genesis_event(upgrade_wire(TARGET)))
        .await
        .expect("a no-op");

    assert_eq!(upgrades.creations(), 1);
}

#[tokio::test]
async fn a_different_version_while_one_is_in_flight_is_refused() {
    let upgrades = Arc::new(InMemoryUpgrades::default());
    let handler = handler(&upgrades);
    handler
        .handle(genesis_event(upgrade_wire(TARGET)))
        .await
        .expect("created");

    let error = handler
        .handle(genesis_event(upgrade_wire("27.0.0")))
        .await
        .expect_err("one is in flight");

    assert!(error.to_string().contains(TARGET), "{error}");
    assert_eq!(upgrades.creations(), 1);
    assert_eq!(
        upgrades.desired(&dataplane().to_string()).target_version,
        TARGET
    );
}

#[test]
fn genesis_receives_the_routing_key_the_control_plane_gives_a_data_plane_upgrade() {
    let upgrades = Arc::new(InMemoryUpgrades::default());
    let key = handler(&upgrades).routing_key().to_string();

    assert_eq!(key, aether_domain::action::DATAPLANE_UPGRADE_ACTION_TYPE);
    assert!(
        genesis_core::infrastructure::rabbitmq::consumer::is_bound(&key),
        "Herald publishes under '{key}' and the broker drops it unless Genesis' queue is bound to it"
    );
}
