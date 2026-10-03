mod bdd {
    pub mod iam_settings;
    pub mod plan_features;
    pub mod reachability;
    pub mod signals;
    pub mod upgrade;
    pub mod upgrade_request;
}

use bdd::iam_settings::IamSettingsWorld;
use bdd::plan_features::PlanFeaturesWorld;
use bdd::reachability::ReachabilityWorld;
use bdd::signals::SignalsWorld;
use bdd::upgrade::UpgradeWorld;
use bdd::upgrade_request::UpgradeRequestWorld;
use cucumber::{StatsWriter, World};

fn not_wip(
    _: &cucumber::gherkin::Feature,
    _: Option<&cucumber::gherkin::Rule>,
    scenario: &cucumber::gherkin::Scenario,
) -> bool {
    !scenario.tags.iter().any(|tag| tag == "wip")
}

#[tokio::test]
async fn the_data_plane_upgrade_scenarios_pass() {
    let summary = UpgradeWorld::cucumber()
        .with_default_cli()
        .fail_on_skipped()
        .filter_run("tests/features/upgrade", not_wip)
        .await;

    assert!(!summary.execution_has_failed(), "{summary:?}");
}

#[tokio::test]
async fn the_reachability_scenarios_pass() {
    let summary = ReachabilityWorld::cucumber()
        .with_default_cli()
        .fail_on_skipped()
        .filter_run("tests/features/reachability", not_wip)
        .await;

    assert!(!summary.execution_has_failed(), "{summary:?}");
}

#[tokio::test]
async fn the_data_plane_upgrade_request_scenarios_pass() {
    let summary = UpgradeRequestWorld::cucumber()
        .with_default_cli()
        .fail_on_skipped()
        .filter_run("tests/features/dataplane_upgrade_request", not_wip)
        .await;

    assert!(!summary.execution_has_failed(), "{summary:?}");
}

#[tokio::test]
async fn the_signals_scenarios_pass() {
    let summary = SignalsWorld::cucumber()
        .with_default_cli()
        .fail_on_skipped()
        .filter_run("tests/features/signals", not_wip)
        .await;

    assert!(!summary.execution_has_failed(), "{summary:?}");
}

#[tokio::test]
async fn the_plan_features_scenarios_pass() {
    let summary = PlanFeaturesWorld::cucumber()
        .with_default_cli()
        .fail_on_skipped()
        .filter_run("tests/features/plans", not_wip)
        .await;

    assert!(!summary.execution_has_failed(), "{summary:?}");
}

#[tokio::test]
async fn the_iam_settings_scenarios_pass() {
    let summary = IamSettingsWorld::cucumber()
        .with_default_cli()
        .fail_on_skipped()
        .filter_run("tests/features/iam_settings", not_wip)
        .await;

    assert!(!summary.execution_has_failed(), "{summary:?}");
}
