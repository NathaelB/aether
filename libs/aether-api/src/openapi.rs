use utoipa::OpenApi;

use crate::handlers::{
    actions::ActionApiDoc,
    audit::AuditApiDoc,
    dataplanes::DataPlaneApiDoc,
    deployments::DeploymentApiDoc,
    metrics::{MetricsApiDoc, MetricsIngestApiDoc},
    organisations::OrganisationApiDoc,
    regions::RegionApiDoc,
    releases::ReleaseApiDoc,
    roles::RoleApiDoc,
    users::UserApiDoc,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Aether API",
        version = "0.1.0",
        description = "API documentation for Aether services"
    ),
    nest(
        (path = "/organisations", api = OrganisationApiDoc),
        (path = "/organisations", api = RoleApiDoc),
        (path = "/organisations", api = DeploymentApiDoc),
        (path = "/organisations", api = ActionApiDoc),
        (path = "/organisations", api = AuditApiDoc),
        (path = "/organisations", api = MetricsApiDoc),
        (path = "/deployments", api = MetricsIngestApiDoc),
        (path = "/users", api = UserApiDoc),
        (path = "/dataplanes", api = DataPlaneApiDoc),
        (path = "/regions", api = RegionApiDoc),
        (path = "/releases", api = ReleaseApiDoc),
    )
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::ApiDoc;
    use utoipa::OpenApi;

    #[test]
    fn openapi_has_title() {
        let doc = ApiDoc::openapi();
        assert_eq!(doc.info.title, "Aether API");
    }

    /// The committed document must match the handlers.
    ///
    /// `openapi.json` is what the console's API client is generated from, so a
    /// handler change that never reaches it is a change the console cannot
    /// see. That is not hypothetical: the client had drifted to the point of
    /// carrying five of `CreateDeploymentRequest`'s ten fields, and the create
    /// form silently dropped the region and the sizing a user had chosen.
    ///
    /// Failing here rather than in the console is deliberate -- this is the
    /// side that changed, and the fix belongs in the same commit as the change.
    #[test]
    fn the_committed_openapi_document_is_up_to_date() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("aether-api lives two directories below the repository root");
        let path = repo_root.join("openapi.json");

        let committed = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let current = ApiDoc::openapi()
            .to_pretty_json()
            .expect("the document serializes");

        if committed.trim() == current.trim() {
            return;
        }

        // Not `assert_eq!`: the document is 70+ kB, and dumping both copies
        // into a CI log buries the one line that actually differs. Point at
        // that line instead, and leave reading the rest to the diff.
        let first_difference = committed
            .lines()
            .zip(current.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b)
            .map(|(line, (committed, current))| {
                format!(
                    "line {}:\n  committed: {}\n  current:   {}",
                    line + 1,
                    committed.trim(),
                    current.trim()
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "one document is a prefix of the other: {} committed lines vs {} current",
                    committed.lines().count(),
                    current.lines().count()
                )
            });

        panic!(
            "openapi.json is stale -- run ./scripts/generate-api-client.sh and commit the result\n\n{first_difference}"
        );
    }
}

/// Every route the server actually serves, taken from the `TypedPath` that
/// defines it rather than from the documentation that describes it.
///
/// The two used to disagree: seven operator routes answered on
/// `/operator/releases/...` while the document said `/releases/operator/...`.
/// The console's client is generated from the document, so the release
/// catalogue called URLs that did not exist and came back empty, with nothing
/// failing anywhere a test could see it.
#[cfg(test)]
fn served_paths() -> Vec<&'static str> {
    use axum_extra::routing::TypedPath;

    use crate::handlers::{
        dataplanes, deployments, metrics, organisations, releases, roles, users,
    };

    vec![
        <organisations::create_organisation::CreateOrganisationRoute as TypedPath>::PATH,
        <organisations::get_organisations::GetOrganisationsRoute as TypedPath>::PATH,
        <deployments::list_deployments::ListDeploymentsRoute as TypedPath>::PATH,
        <deployments::create_deployment::CreateDeploymentRoute as TypedPath>::PATH,
        <deployments::get_deployment::GetDeploymentRoute as TypedPath>::PATH,
        <deployments::update_deployment::UpdateDeploymentRoute as TypedPath>::PATH,
        <deployments::delete_deployment::DeleteDeploymentRoute as TypedPath>::PATH,
        <deployments::upgrade_deployment::UpgradeDeploymentRoute as TypedPath>::PATH,
        <deployments::upgrade_in_flight::UpgradeInFlightRoute as TypedPath>::PATH,
        <deployments::upgrade_settings::UpgradeSettingsRoute as TypedPath>::PATH,
        <deployments::network_access::NetworkAccessRoute as TypedPath>::PATH,
        <deployments::read_logs::ReadLogsRoute as TypedPath>::PATH,
        <releases::list_releases::ListReleasesRoute as TypedPath>::PATH,
        <releases::list_releases::ListReleasesForOperatorRoute as TypedPath>::PATH,
        <releases::publish_release::PublishReleaseRoute as TypedPath>::PATH,
        <releases::publish_release::ReviseReleaseRoute as TypedPath>::PATH,
        <releases::publish_release::MoveReleaseRoute as TypedPath>::PATH,
        <releases::rollout::WidenRolloutRoute as TypedPath>::PATH,
        <releases::rollout::PreviewRolloutRoute as TypedPath>::PATH,
        <releases::rollout::ReleaseHoldBacksRoute as TypedPath>::PATH,
        <releases::rollout::ReleaseAvailabilityRoute as TypedPath>::PATH,
        <metrics::get_deployment_usage::GetDeploymentUsageRoute as TypedPath>::PATH,
        <metrics::get_active_users::GetActiveUsersRoute as TypedPath>::PATH,
        <metrics::report_usage_metrics::ReportUsageMetricsRoute as TypedPath>::PATH,
        <dataplanes::push_logs::PushLogsRoute as TypedPath>::PATH,
        <roles::create_role::CreateRoleRoute as TypedPath>::PATH,
        <users::get_user_organisations::GetUserOrganisationsRoute as TypedPath>::PATH,
    ]
}

#[cfg(test)]
mod route_agreement {
    use super::{ApiDoc, served_paths};
    use utoipa::OpenApi;

    /// A documented path nothing serves is a 404 waiting for whoever generated
    /// a client from it, and nothing in the server notices.
    #[test]
    fn every_route_is_documented_where_it_actually_answers() {
        let documented: Vec<String> = ApiDoc::openapi().paths.paths.keys().cloned().collect();

        let missing: Vec<&str> = served_paths()
            .into_iter()
            .filter(|path| !documented.iter().any(|doc| doc == path))
            .collect();

        assert!(
            missing.is_empty(),
            "these routes answer on a path the document does not describe: {missing:#?}\n\
             documented paths: {documented:#?}"
        );
    }
}
