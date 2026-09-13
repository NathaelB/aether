//! Answering what an organisation may buy.
//!
//! Its own port rather than a method on `OrganisationService`. That trait is
//! implemented by a pure domain service which has no policy and no business
//! knowing about tiers opening offers; adding this to it would force an
//! implementation that exists only to satisfy a signature.

use std::future::Future;

use aether_auth::Identity;

use crate::{CoreError, offers::OfferAvailability, organisation::OrganisationId};

pub trait OfferService: Send + Sync {
    /// The whole catalogue, answered for this organisation's tier.
    ///
    /// Gated on being able to see instances: the caller asking is the one
    /// about to place one, and the answer discloses which tier they are on.
    fn list_offers(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Vec<OfferAvailability>, CoreError>> + Send;
}
