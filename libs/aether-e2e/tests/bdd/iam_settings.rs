use std::sync::Arc;

use aether_crds::v1alpha::identity_instance::Branding as ResourceBranding;
use aether_domain::CoreError;
use aether_domain::deployments::DeploymentId;
use aether_domain::iam_settings::branding::Branding;
use aether_domain::iam_settings::ports::{IamSettingsService, IamSettingsState};
use aether_domain::iam_settings::service::IamSettingsServiceImpl;
use aether_domain::organisation::OrganisationId;
use aether_domain::organisation::features::IamFeature;
use aether_domain::organisation::value_objects::Plan;
use aether_e2e::support::iam::{
    DEPLOYMENT, InMemoryDeployments, InstanceRights, ORGANISATION, OneOrganisation, SpyInstances,
    branding_from, deployment_of, domain_colors, event_of, iam_settings_actions,
    resource_branding_of, resource_colors, settings_of,
};
use aether_e2e::support::platform::caller;
use aether_e2e::support::requests::InMemoryActions;
use aether_operator_core::domain::identity_instance::theme::{
    ThemeDecision, decide, marker, theme_config,
};
use cucumber::{World, given, then, when};
use genesis_core::application::handlers::iam_settings::IamSettingsEventHandler;
use genesis_core::domain::error::GenesisError;
use genesis_core::domain::ports::EventHandler;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct IamSettingsWorld {
    deployments: InMemoryDeployments,
    actions: InMemoryActions,
    rights: InstanceRights,
    plan_closes_branding: bool,
    refusal: Option<String>,
    outcome: Option<Result<IamSettingsState, CoreError>>,
    instances: Arc<SpyInstances>,
    genesis: Vec<Result<(), GenesisError>>,
    operator_branding: Option<ResourceBranding>,
    config: Value,
    decision: Option<String>,
}

impl IamSettingsWorld {
    fn new() -> Self {
        Self {
            deployments: InMemoryDeployments::holding(deployment_of(ORGANISATION)),
            actions: InMemoryActions::default(),
            rights: InstanceRights::to_manage(),
            plan_closes_branding: false,
            refusal: None,
            outcome: None,
            instances: Arc::default(),
            genesis: Vec::new(),
            operator_branding: None,
            config: Value::Null,
            decision: None,
        }
    }

    async fn submit(&mut self, branding: Option<Branding>) {
        let service = IamSettingsServiceImpl::new(
            self.deployments.clone(),
            self.actions.clone(),
            OneOrganisation::on(Plan::Free),
            self.rights,
        );
        let organisation = OrganisationId(ORGANISATION);
        let deployment = DeploymentId(DEPLOYMENT);
        let settings = settings_of(branding);

        self.outcome = Some(if self.plan_closes_branding {
            service
                .set_when(
                    caller(),
                    organisation,
                    deployment,
                    settings,
                    |_, feature| feature != IamFeature::Branding,
                )
                .await
        } else {
            service
                .set_iam_settings(caller(), organisation, deployment, settings)
                .await
        });
    }

    async fn save(&mut self, wire: &str) {
        match branding_from(wire) {
            Ok(branding) => self.submit(Some(branding)).await,
            Err(error) => self.refusal = Some(error.to_string()),
        }
    }

    async fn genesis_applies(&mut self, times: usize) {
        let actions = self.actions.all();
        let action = *iam_settings_actions(&actions).last().expect("an action");
        let handler = IamSettingsEventHandler::new(self.instances.clone());
        for _ in 0..times {
            self.genesis.push(handler.handle(event_of(action)).await);
        }
    }

    fn last_patch(&self) -> Value {
        self.instances
            .patches()
            .last()
            .unwrap_or_else(|| panic!("Genesis wrote nothing: {:?}", self.genesis))
            .1
            .clone()
    }

    fn last_payload(&self) -> Value {
        let actions = self.actions.all();
        iam_settings_actions(&actions)
            .last()
            .expect("an action")
            .payload
            .data
            .clone()
    }
}

fn json_of(wire: &str) -> Value {
    serde_json::from_str(wire).expect("json in the scenario")
}

#[given("a deployment of the organisation")]
fn a_deployment_of_the_organisation(world: &mut IamSettingsWorld) {
    world.deployments = InMemoryDeployments::holding(deployment_of(ORGANISATION));
}

#[given("a deployment of another organisation")]
fn a_deployment_of_another_organisation(world: &mut IamSettingsWorld) {
    world.deployments = InMemoryDeployments::holding(deployment_of(Uuid::from_u128(99)));
}

#[given("a caller who may view instances but not manage them")]
fn a_caller_who_may_only_view(world: &mut IamSettingsWorld) {
    world.rights = InstanceRights::to_view_only();
}

#[given("a plan that closes branding")]
fn a_plan_that_closes_branding(world: &mut IamSettingsWorld) {
    world.plan_closes_branding = true;
}

#[given(expr = "the customer has saved the branding {string}")]
async fn the_customer_has_saved(world: &mut IamSettingsWorld, wire: String) {
    world.save(&wire).await;
}

#[given("the customer has cleared the branding")]
async fn the_customer_has_cleared(world: &mut IamSettingsWorld) {
    world.submit(None).await;
}

#[given(expr = "the resource's branding is {string}")]
fn the_resources_branding_is(world: &mut IamSettingsWorld, wire: String) {
    world.operator_branding = serde_json::from_str(&wire).expect("a resource branding");
}

#[when(expr = "the customer saves the branding {string}")]
async fn the_customer_saves(world: &mut IamSettingsWorld, wire: String) {
    world.save(&wire).await;
}

#[when("the customer clears the branding")]
async fn the_customer_clears(world: &mut IamSettingsWorld) {
    world.submit(None).await;
}

#[when("Genesis applies the action the control plane created")]
async fn genesis_applies_the_action(world: &mut IamSettingsWorld) {
    world.genesis_applies(1).await;
}

#[when("Genesis applies the action the control plane created twice")]
async fn genesis_applies_the_action_twice(world: &mut IamSettingsWorld) {
    world.genesis_applies(2).await;
}

#[when("the operator maps it to the theme configuration")]
fn the_operator_maps_it(world: &mut IamSettingsWorld) {
    let branding = world.operator_branding.as_ref().expect("a branding");
    world.config = theme_config(branding);
}

#[when(expr = "the operator decides with a {string} marker")]
fn the_operator_decides(world: &mut IamSettingsWorld, which: String) {
    let branding = world.operator_branding.as_ref();
    let applied = match which.as_str() {
        "none" => None,
        "other" => Some("something else".to_string()),
        "same" => branding.map(marker),
        unknown => panic!("unknown marker {unknown}"),
    };
    world.decision = Some(
        match decide(branding, applied.as_deref()) {
            ThemeDecision::Apply(_) => "apply",
            ThemeDecision::RestoreDefault => "restore the default",
            ThemeDecision::Nothing => "nothing",
        }
        .to_string(),
    );
}

#[then("the change is accepted")]
fn the_change_is_accepted(world: &mut IamSettingsWorld) {
    let outcome = world.outcome.as_ref().expect("a change was attempted");
    assert!(outcome.is_ok(), "{outcome:?}");
}

#[then("the change is refused as permission denied")]
fn the_change_is_refused(world: &mut IamSettingsWorld) {
    let outcome = world.outcome.as_ref().expect("a change was attempted");
    assert!(
        matches!(outcome, Err(CoreError::PermissionDenied { .. })),
        "{outcome:?}"
    );
}

#[then("the deployment is reported as not found")]
fn the_deployment_is_not_found(world: &mut IamSettingsWorld) {
    let outcome = world.outcome.as_ref().expect("a change was attempted");
    assert!(
        matches!(outcome, Err(CoreError::DeploymentNotFound { .. })),
        "{outcome:?}"
    );
}

#[then("the branding is refused")]
fn the_branding_is_refused(world: &mut IamSettingsWorld) {
    assert!(world.refusal.is_some(), "the branding was accepted");
    assert!(world.outcome.is_none(), "the service was reached");
}

#[then("nothing is stored and no action is created")]
fn nothing_is_stored(world: &mut IamSettingsWorld) {
    assert_eq!(world.deployments.updates(), 0);
    assert_eq!(world.deployments.branding(), None);
    assert!(world.actions.all().is_empty());
}

#[then(expr = "the deployment stores the branding {string}")]
fn the_deployment_stores(world: &mut IamSettingsWorld, wire: String) {
    let stored = world.deployments.branding().expect("a stored branding");
    assert_eq!(serde_json::to_value(stored).unwrap(), json_of(&wire));
}

#[then("the deployment stores no branding")]
fn the_deployment_stores_none(world: &mut IamSettingsWorld) {
    assert_eq!(world.deployments.branding(), None);
}

#[then(expr = "exactly one {string} action is created")]
fn exactly_one_action(world: &mut IamSettingsWorld, kind: String) {
    let actions = world.actions.all();
    assert_eq!(actions.len(), 1, "{actions:?}");
    assert_eq!(actions[0].action_type.0, kind);
}

#[then(expr = "the action payload has exactly the keys {string}, {string} and {string}")]
fn the_payload_has_keys(world: &mut IamSettingsWorld, a: String, b: String, c: String) {
    let payload = world.last_payload();
    let mut keys: Vec<&str> = payload
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec![a.as_str(), b.as_str(), c.as_str()]);
}

#[then(expr = "the action carries the branding {string}")]
fn the_action_carries(world: &mut IamSettingsWorld, wire: String) {
    assert_eq!(world.last_payload()["branding"], json_of(&wire));
}

#[then("the latest action carries a null branding")]
fn the_action_carries_null(world: &mut IamSettingsWorld) {
    let payload = world.last_payload();
    assert!(payload["branding"].is_null(), "{payload}");
    assert!(
        payload
            .as_object()
            .expect("an object")
            .contains_key("branding")
    );
}

#[then("Genesis accepted the event")]
fn genesis_accepted(world: &mut IamSettingsWorld) {
    assert!(!world.genesis.is_empty(), "Genesis was not asked");
    for result in &world.genesis {
        assert!(result.is_ok(), "{result:?}");
    }
}

#[then("the instance written is the deployment's own")]
fn the_instance_is_the_deployments(world: &mut IamSettingsWorld) {
    let patches = world.instances.patches();
    let reference = &patches.last().expect("a write").0;
    assert_eq!(reference.name, format!("deployment-{DEPLOYMENT}"));
    assert_eq!(reference.namespace, "production-auth");
}

#[then("the resource holds the same colors and radius as the stored branding")]
fn the_resource_holds_the_same(world: &mut IamSettingsWorld) {
    let stored = world.deployments.branding().expect("a stored branding");
    let resource = resource_branding_of(&world.last_patch()).expect("a resource branding");

    let expected = domain_colors(&stored);
    let written = resource_colors(&resource);
    for (expected, written) in expected.iter().zip(&written) {
        assert_eq!(
            expected.as_ref().map(|color| color.to_lowercase()),
            written.as_ref().map(|color| color.to_lowercase())
        );
    }
    assert_eq!(resource.radius, stored.radius.map(|radius| radius.get()));
}

#[then(expr = "the resource holds {string} at colors {string}")]
fn the_resource_holds_at(world: &mut IamSettingsWorld, color: String, key: String) {
    let patch = world.last_patch();
    assert_eq!(
        patch["spec"]["iam"]["branding"]["colors"][&key],
        json!(color),
        "{patch}"
    );
}

#[then("the resource's iam is cleared")]
fn the_resources_iam_is_cleared(world: &mut IamSettingsWorld) {
    assert_eq!(world.last_patch(), json!({ "spec": { "iam": null } }));
}

#[then("both writes are the same patch")]
fn both_writes_are_the_same(world: &mut IamSettingsWorld) {
    let patches = world.instances.patches();
    assert_eq!(patches.len(), 2);
    assert_eq!(patches[0], patches[1]);
}

#[then(expr = "the theme configuration is {string}")]
fn the_theme_configuration_is(world: &mut IamSettingsWorld, wire: String) {
    assert_eq!(world.config, json_of(&wire));
}

#[then(expr = "the decision is {string}")]
fn the_decision_is(world: &mut IamSettingsWorld, expected: String) {
    assert_eq!(world.decision.as_deref(), Some(expected.as_str()));
}
