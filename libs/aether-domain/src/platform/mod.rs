//! What the installation is running, seen from outside every organisation.
//!
//! A different question from the ones the other modules answer, and
//! deliberately a different module. `deployments` answers "what does this
//! organisation have", authorised as a member of it; this answers "what is on
//! this installation", authorised as somebody who runs it. Returning the first
//! to whoever passes an organisation id is the bug that this separation exists
//! to make impossible to write by accident.

pub mod ports;
pub mod rights;
pub mod service;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

pub use rights::{PlatformRight, PlatformRights};

use crate::{
    dataplane::value_objects::{DataPlaneId, Region},
    deployments::{Deployment, DeploymentId, DeploymentStatus},
    organisation::{Organisation, OrganisationId, value_objects::OrganisationStatus},
};

/// Somebody who operates this installation, and what they were granted.
///
/// Keyed on the subject the identity provider issues, because that is the one
/// thing a token carries that identifies a caller and cannot be renamed out
/// from under a grant. It covers clients as well as people: registering a data
/// plane is done by `aether-operator-cli`, which is not somebody.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct PlatformOperator {
    pub subject: String,
    pub rights: PlatformRights,

    /// The subject who granted these, or `None` for the one the installation
    /// named at startup. Nobody granted the first operator; that is what
    /// bootstrapping means, and recording a lie about it would be worse than
    /// recording the gap.
    pub granted_by: Option<String>,
    pub granted_at: DateTime<Utc>,
}

/// The bounds on one page of the estate.
///
/// Named rather than three loose arguments: `limit` and a cursor that is also
/// a uuid have been swapped before, and a call site that swapped them here
/// would page from a limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstateQuery {
    pub organisation: Option<OrganisationId>,
    pub dataplane: Option<DataPlaneId>,
    pub region: Option<Region>,
    pub status: Option<DeploymentStatus>,
    pub limit: usize,

    /// The last deployment of the previous page.
    ///
    /// Keyset rather than an offset: the estate is ordered newest first and a
    /// deployment created while somebody pages would shift every offset after
    /// it, showing one row twice and skipping another.
    pub cursor: Option<DeploymentId>,
}

/// The most rows one request may ask for.
///
/// A ceiling rather than a suggestion. The screen this serves shows tens of
/// rows; a caller asking for a hundred thousand is either wrong or reading the
/// whole estate one request at a time, and both are better answered by paging.
pub const MAX_PAGE: usize = 200;
const DEFAULT_PAGE: usize = 50;

/// Reads a page's bounds, refusing what cannot be honoured rather than
/// quietly substituting something else.
///
/// One reader for both listings. Two would be two ceilings, and the one nobody
/// remembered to raise is the one somebody hits.
fn page_of(limit: Option<usize>) -> Result<usize, String> {
    match limit {
        None => Ok(DEFAULT_PAGE),
        Some(0) => Err("a page of nothing is not a page".to_string()),
        Some(asked) if asked > MAX_PAGE => Err(format!("a page may hold at most {MAX_PAGE} rows")),
        Some(asked) => Ok(asked),
    }
}

impl EstateQuery {
    pub fn new(limit: Option<usize>, cursor: Option<DeploymentId>) -> Result<Self, String> {
        let limit = page_of(limit)?;

        Ok(Self {
            organisation: None,
            dataplane: None,
            region: None,
            status: None,
            limit,
            cursor,
        })
    }

    pub fn in_organisation(mut self, organisation: Option<OrganisationId>) -> Self {
        self.organisation = organisation;
        self
    }

    pub fn on_dataplane(mut self, dataplane: Option<DataPlaneId>) -> Self {
        self.dataplane = dataplane;
        self
    }

    pub fn in_region(mut self, region: Option<Region>) -> Self {
        self.region = region;
        self
    }

    pub fn with_status(mut self, status: Option<DeploymentStatus>) -> Self {
        self.status = status;
        self
    }
}

/// Who owns a deployment, as much of it as a list needs.
///
/// The name travels because the alternative is a screen resolving one
/// organisation per row, and an estate of two hundred deployments would open
/// two hundred requests to draw one page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct EstateOwner {
    pub id: OrganisationId,
    pub name: String,
}

/// One deployment on the installation, and enough about it to act.
///
/// The deployment travels whole rather than field by field. It already carries
/// its kind, version, status, environment and offer, and a flattened copy here
/// would be a second definition of a deployment that drifts from the first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct EstateDeployment {
    pub deployment: Deployment,
    pub organisation: EstateOwner,

    /// Where the data plane it sits on runs. Read from the data plane rather
    /// than from the deployment, which does not carry one: a deployment is
    /// placed in a region by being placed on a cluster.
    pub region: Region,
}

/// The bounds on one page of the tenants.
///
/// Its own type rather than a reuse of [`EstateQuery`]: the two filter on
/// different things, and a shared struct would carry a region field that means
/// nothing about an organisation and a status field whose values are not the
/// same set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantQuery {
    pub status: Option<OrganisationStatus>,
    pub limit: usize,
    pub cursor: Option<OrganisationId>,
}

impl TenantQuery {
    pub fn new(limit: Option<usize>, cursor: Option<OrganisationId>) -> Result<Self, String> {
        Ok(Self {
            status: None,
            limit: page_of(limit)?,
            cursor,
        })
    }

    pub fn with_status(mut self, status: Option<OrganisationStatus>) -> Self {
        self.status = status;
        self
    }
}

/// One organisation on the installation, and what it holds.
///
/// The two counts travel because they are the first thing anybody looks at:
/// an organisation with no deployments and one member is a trial nobody came
/// back to, and one with forty of each is a conversation before any change.
/// Counted here rather than by the screen, which would open one request per
/// row.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct Tenant {
    pub organisation: Organisation,
    pub deployments: usize,
    pub members: usize,
}

/// One page of the tenants.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct TenantPage {
    pub tenants: Vec<Tenant>,
    pub next_cursor: Option<OrganisationId>,
}

/// One page of the estate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct EstatePage {
    pub deployments: Vec<EstateDeployment>,

    /// `None` when this page is the last one.
    ///
    /// Answered by the query rather than inferred from a short page: a page
    /// that happens to be exactly `limit` long is not evidence that more
    /// exists, and a caller that guessed would ask once too often for ever.
    pub next_cursor: Option<DeploymentId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_nobody_bounded_has_a_default_rather_than_no_bound() {
        assert_eq!(EstateQuery::new(None, None).unwrap().limit, DEFAULT_PAGE);
    }

    /// Both refusals name the bound. A caller reading "invalid limit" has to
    /// guess which direction it was wrong in.
    #[test]
    fn a_page_that_cannot_be_honoured_is_refused_rather_than_resized() {
        assert!(EstateQuery::new(Some(0), None).is_err());

        let too_much = EstateQuery::new(Some(MAX_PAGE + 1), None)
            .expect_err("a page beyond the ceiling was accepted");
        assert!(too_much.contains(&MAX_PAGE.to_string()), "got {too_much}");
    }

    /// Both listings read their bounds the same way. Two readers would be two
    /// ceilings, and the one nobody remembered to raise is the one somebody
    /// hits.
    #[test]
    fn both_listings_bound_a_page_the_same_way() {
        assert_eq!(TenantQuery::new(None, None).unwrap().limit, DEFAULT_PAGE);
        assert!(TenantQuery::new(Some(0), None).is_err());
        assert!(TenantQuery::new(Some(MAX_PAGE + 1), None).is_err());
    }

    #[test]
    fn a_query_with_no_filters_asks_about_the_whole_estate() {
        let whole = EstateQuery::new(None, None).unwrap();

        assert!(whole.organisation.is_none());
        assert!(whole.dataplane.is_none());
        assert!(whole.region.is_none());
        assert!(whole.status.is_none());
    }
}
