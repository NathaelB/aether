//! Publishing where a deployment can be reached.
//!
//! `<deployment>.autharie.fr` cannot be one wildcard record the way the
//! control plane's own hostnames are: a deployment lands on one of
//! potentially many data planes, each its own cluster with its own address
//! (see [`crate::dataplane::entities::DataPlane::gateway_address`]). Routing
//! it needs a record naming that cluster specifically.
//!
//! [`DnsProvider`] is this platform's own shape for "somewhere that turns a
//! hostname into a record" -- OVH today, and whatever the adapter under this
//! port is on the day it is not. Configured like every other optional
//! integration here: absent means the feature does nothing, not that it
//! fails.

use thiserror::Error;

/// What went wrong asking the provider to change a record.
///
/// Two of these are worth telling apart: a provider that did not answer at
/// all is worth retrying, one that answered and refused is not -- an installer
/// waiting on a hostname needs to know which.
#[derive(Debug, Clone, Error)]
pub enum DnsError {
    #[error("the DNS provider is unreachable: {reason}")]
    ProviderUnavailable { reason: String },

    #[error("{operation} was refused: {reason}")]
    Refused { operation: String, reason: String },

    #[error("the DNS provider returned something this platform cannot read: {reason}")]
    Malformed { reason: String },
}

/// Somewhere that turns a hostname into a record pointing at an address.
///
/// One provider, one zone: an adapter is constructed for the zone it
/// publishes into, which is why `zone` is a method here rather than a
/// parameter on every call -- there is nowhere a caller could get a second
/// one from.
pub trait DnsProvider: Send + Sync {
    /// The zone this provider publishes records in, e.g. `autharie.fr`.
    fn zone(&self) -> &str;

    /// Creates or updates the record so `hostname` resolves to `target`.
    ///
    /// Idempotent: called again with the address it already carries is a
    /// no-op, which is what lets a reconciliation sweep call this on every
    /// deployment it knows about rather than tracking which ones changed.
    fn upsert_record(
        &self,
        hostname: &str,
        target: &str,
    ) -> impl Future<Output = Result<(), DnsError>> + Send;

    /// Removes the record for `hostname`.
    ///
    /// Idempotent: no matching record is the outcome asked for, not an error
    /// -- a deployment deleted before its data plane ever reported an address
    /// never had one to remove.
    fn delete_record(&self, hostname: &str) -> impl Future<Output = Result<(), DnsError>> + Send;
}

/// A deployment's own hostname, under whichever zone this installation
/// publishes DNS records in.
///
/// The deployment's name alone, not the namespace `namespace_for` derives:
/// a hostname is what a customer sees and shares, and the discriminator that
/// keeps namespaces unique across organisations has no place in it.
pub fn hostname_for(deployment_name: &str, zone: &str) -> String {
    format!(
        "{}.{zone}",
        crate::deployments::environment::slug(deployment_name)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hostname_is_the_slugged_name_under_the_zone() {
        assert_eq!(
            hostname_for("My Deployment", "autharie.fr"),
            "my-deployment.autharie.fr"
        );
    }

    #[test]
    fn a_name_that_is_already_a_valid_label_is_left_alone() {
        assert_eq!(
            hostname_for("acme-prod", "autharie.fr"),
            "acme-prod.autharie.fr"
        );
    }
}
