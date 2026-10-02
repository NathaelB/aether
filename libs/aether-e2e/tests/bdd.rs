mod bdd {
    pub mod reachability;
    pub mod upgrade;
}

use bdd::reachability::ReachabilityWorld;
use bdd::upgrade::UpgradeWorld;
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
