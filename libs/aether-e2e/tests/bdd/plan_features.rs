use aether_domain::organisation::features::{
    IamFeature, PlanFeatures, cheapest_plan_opening, features_for,
};
use aether_domain::organisation::value_objects::Plan;
use cucumber::{World, given, then, when};

#[derive(Debug, Default, World)]
pub struct PlanFeaturesWorld {
    plan: Option<Plan>,
    answer: Option<PlanFeatures>,
    set: Vec<IamFeature>,
    cheapest: Option<Option<Plan>>,
}

fn plan(name: &str) -> Plan {
    name.parse().expect("a known plan")
}

fn feature(name: &str) -> IamFeature {
    serde_json::from_value(serde_json::Value::String(name.to_string())).expect("a known feature")
}

#[given(expr = "an organisation on the {string} plan")]
fn an_organisation_on_a_plan(world: &mut PlanFeaturesWorld, name: String) {
    world.plan = Some(plan(&name));
}

#[when("its features are listed")]
fn its_features_are_listed(world: &mut PlanFeaturesWorld) {
    let plan = world.plan.expect("an organisation");
    world.answer = Some(features_for(plan));
}

#[when("the feature set is listed")]
fn the_feature_set_is_listed(world: &mut PlanFeaturesWorld) {
    world.set = IamFeature::ALL.to_vec();
}

#[when(expr = "the cheapest plan opening {string} is asked")]
fn the_cheapest_plan_is_asked(world: &mut PlanFeaturesWorld, name: String) {
    world.cheapest = Some(cheapest_plan_opening(feature(&name)));
}

#[then(expr = "{string} is open")]
fn a_feature_is_open(world: &mut PlanFeaturesWorld, name: String) {
    let wanted = feature(&name);
    let entry = world
        .answer
        .as_ref()
        .expect("features were listed")
        .features
        .iter()
        .find(|entry| entry.feature == wanted)
        .expect("the feature is answered");

    assert!(entry.open, "{name} is closed");
}

#[then(expr = "{string} is open with no unlocking tier")]
fn a_feature_is_open_with_no_tier(world: &mut PlanFeaturesWorld, name: String) {
    let wanted = feature(&name);
    let entry = world
        .answer
        .as_ref()
        .expect("features were listed")
        .features
        .iter()
        .find(|entry| entry.feature == wanted)
        .expect("the feature is answered");

    assert!(entry.open, "{name} is closed");
    assert_eq!(entry.opened_by, None);
}

#[then("the features are:")]
fn the_features_are(world: &mut PlanFeaturesWorld, step: &cucumber::gherkin::Step) {
    let table = step.table.as_ref().expect("a table of features");
    let expected: Vec<IamFeature> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| feature(&row[0]))
        .collect();

    assert_eq!(world.set, expected);
}

#[then(expr = "the answer is the {string} plan")]
fn the_answer_is_the_plan(world: &mut PlanFeaturesWorld, name: String) {
    assert_eq!(world.cheapest, Some(Some(plan(&name))));
}
