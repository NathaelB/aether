use utoipa::OpenApi;

use crate::handlers::{
    actions::ActionApiDoc, dataplanes::DataPlaneApiDoc, deployments::DeploymentApiDoc,
    organisations::OrganisationApiDoc, roles::RoleApiDoc, users::UserApiDoc,
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
        (path = "/users", api = UserApiDoc),
        (path = "/dataplanes", api = DataPlaneApiDoc),
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
