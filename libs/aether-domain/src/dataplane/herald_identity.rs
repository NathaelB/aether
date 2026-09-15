//! How a data plane proves which data plane it is.
//!
//! Every cluster used to authenticate as the same client, with the same secret,
//! and name the data plane it was acting for in the URL. So the control plane
//! knew it was talking to *a* Herald and took its word for *which* -- which
//! meant one compromised cluster could claim another's work, acknowledge it
//! unfinished, keep a dead cluster eligible for placement, and report a live
//! deployment deleted.
//!
//! A cluster now gets an identity of its own, and the data plane it may speak
//! for is read from that identity rather than from anything it sends.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The identity provider's name for a data plane's Herald, and the subject its
/// tokens carry.
///
/// Both, because they answer different questions. The client id is what a
/// human reads in the realm and what the chart is configured with; the subject
/// is what a token actually says, and the only one an authorisation decision
/// may use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HeraldBinding {
    /// `herald-<data plane id>`, for somebody looking at the realm.
    pub client_id: String,

    /// The subject of the service account behind that client.
    ///
    /// This is the binding. `preferred_username` would have worked too and is
    /// what the old substring check looked at, but `service-account-<name>` is
    /// the identity provider's naming convention rather than ours -- building
    /// authorisation on it means every data plane loses access the day that
    /// convention changes.
    pub subject: String,
}

/// A freshly minted identity, secret included.
///
/// The secret exists here and nowhere else. It is returned to whoever
/// registered the data plane, once, and never stored: the control plane keeps
/// what identifies the cluster, not what lets somebody be it.
#[derive(Debug, Clone)]
pub struct MintedHeraldIdentity {
    pub binding: HeraldBinding,
    pub secret: String,
}

impl MintedHeraldIdentity {
    /// What a client is called for a given data plane.
    ///
    /// Derived rather than free, so somebody reading the realm can tell which
    /// cluster a client belongs to without a lookup. Nothing authorises on
    /// this -- see [`HeraldBinding::subject`].
    pub fn client_id_for(dataplane: crate::dataplane::value_objects::DataPlaneId) -> String {
        format!("herald-{}", dataplane.0)
    }
}

/// A data plane and, when one was just minted for it, the secret its Herald
/// authenticates with.
///
/// The secret travels exactly once, in the answer to the request that caused
/// it to exist. Nothing stores it, so an installation that loses it re-issues
/// rather than looks it up -- which is also how a leaked one is dealt with.
#[derive(Debug, Clone)]
pub struct RegisteredDataPlane {
    pub dataplane: crate::dataplane::entities::DataPlane,

    /// `None` when this installation has no realm administrator configured and
    /// therefore cannot mint identities. Said in the answer rather than
    /// failing the registration: a data plane without one is what every
    /// installation had until now.
    pub herald_secret: Option<String>,
}

/// Proof that the caller is the Herald of a particular data plane.
///
/// The field is private and there is no constructor: the only way to hold one
/// is [`speaking_for`], which reads it from the credential. So a service that
/// takes this cannot be handed a data plane id somebody chose -- which is
/// exactly what every one of these checks used to do, comparing an id from the
/// URL against an id from the same URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeraldSpeaking {
    dataplane: crate::dataplane::value_objects::DataPlaneId,
}

impl HeraldSpeaking {
    pub fn dataplane(&self) -> crate::dataplane::value_objects::DataPlaneId {
        self.dataplane
    }

    /// Refuses unless this is the data plane the caller speaks for.
    ///
    /// For the endpoints that name one in their path. The path is still read,
    /// so a Herald asking about the wrong cluster is told rather than quietly
    /// served its own.
    pub fn is(
        &self,
        dataplane: crate::dataplane::value_objects::DataPlaneId,
    ) -> Result<(), crate::CoreError> {
        if self.dataplane == dataplane {
            return Ok(());
        }

        Err(crate::CoreError::PermissionDenied {
            reason: "a data plane may only act for itself".to_string(),
        })
    }
}

/// Which data plane a caller is allowed to speak for.
///
/// Read from the subject its token carries, never from anything it sends.
/// Before this, every cluster authenticated as the same client and named the
/// data plane it was acting for in the URL -- so one compromised cluster could
/// claim another's work, acknowledge it unfinished, keep a dead cluster
/// eligible for placement, and report a live deployment deleted.
pub async fn speaking_for<R: crate::dataplane::ports::DataPlaneRepository>(
    dataplanes: &R,
    identity: &aether_auth::Identity,
) -> Result<HeraldSpeaking, crate::CoreError> {
    let subject = identity.id();

    dataplanes
        .find_by_herald_subject(subject)
        .await?
        .map(|dataplane| HeraldSpeaking {
            dataplane: dataplane.id,
        })
        .ok_or(crate::CoreError::PermissionDenied {
            // Says what is wrong without saying which data planes exist: a
            // caller learning that a subject is unknown to this installation
            // learns nothing it did not already know about itself.
            reason: "only a data plane this installation knows may do this".to_string(),
        })
}

/// Refuses unless this deployment runs on the data plane speaking.
///
/// The other half of the same rule, for the endpoints that name a deployment
/// rather than a cluster. Without it a Herald could claim, acknowledge or
/// report on work belonging to somebody else's cluster.
pub async fn hosting<R: crate::deployments::ports::DeploymentRepository>(
    deployments: &R,
    speaking: &HeraldSpeaking,
    deployment: crate::deployments::DeploymentId,
) -> Result<(), crate::CoreError> {
    let found = deployments
        .get_by_id(deployment)
        .await?
        .ok_or(crate::CoreError::DeploymentNotFound { id: deployment.0 })?;

    speaking.is(found.dataplane_id)
}

/// A provisioner for the tests of services that carry one without exercising
/// it.
///
/// Its own type rather than `Option::None` at every call site, because the
/// generic still has to be named and `None::<Something>` needs a something.
#[cfg(test)]
pub struct NoIdentities;

#[cfg(test)]
impl crate::dataplane::ports::HeraldIdentityProvisioner for NoIdentities {
    async fn mint(
        &self,
        _dataplane: crate::dataplane::value_objects::DataPlaneId,
    ) -> Result<MintedHeraldIdentity, crate::CoreError> {
        unreachable!("a test that mints should say so by carrying a provisioner")
    }

    async fn revoke(
        &self,
        _dataplane: crate::dataplane::value_objects::DataPlaneId,
    ) -> Result<(), crate::CoreError> {
        unreachable!("a test that revokes should say so by carrying a provisioner")
    }
}

#[cfg(test)]
impl HeraldSpeaking {
    /// A proof, for the tests of services that take one.
    ///
    /// Behind `cfg(test)` on purpose: in a build that ships, the only way to
    /// hold one of these is to have read it from a credential.
    pub fn for_test(dataplane: crate::dataplane::value_objects::DataPlaneId) -> Self {
        Self { dataplane }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::dataplane::value_objects::DataPlaneId;

    /// The rule the three "only herald" checks used to each write for
    /// themselves, in two files and three spellings -- one of which accepted
    /// any client whose name merely contained `herald-service`.
    #[tokio::test]
    async fn a_caller_this_installation_does_not_know_speaks_for_nothing() {
        use crate::dataplane::ports::MockDataPlaneRepository;

        let mut dataplanes = MockDataPlaneRepository::new();
        dataplanes
            .expect_find_by_herald_subject()
            .returning(|_| Box::pin(async { Ok(None) }));

        let refused = speaking_for(&dataplanes, &somebody())
            .await
            .expect_err("a stranger spoke for a data plane");

        assert!(matches!(refused, crate::CoreError::PermissionDenied { .. }));
    }

    /// The proof carries the data plane the credential named, and refuses
    /// every other. Before this, both sides of that comparison came from the
    /// caller.
    #[test]
    fn a_data_plane_may_only_act_for_itself() {
        let speaking = HeraldSpeaking::for_test(DataPlaneId(Uuid::from_u128(1)));

        assert!(speaking.is(DataPlaneId(Uuid::from_u128(1))).is_ok());
        assert!(speaking.is(DataPlaneId(Uuid::from_u128(2))).is_err());
    }

    fn somebody() -> aether_auth::Identity {
        aether_auth::Identity::Client(aether_auth::Client {
            id: "whoever".to_string(),
            client_id: "herald-service-lookalike".to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

    /// Two data planes never share a client, which is the whole point: one
    /// secret per cluster means rotating one touches no other.
    #[test]
    fn every_data_plane_is_named_after_itself() {
        let one = MintedHeraldIdentity::client_id_for(DataPlaneId(Uuid::from_u128(1)));
        let other = MintedHeraldIdentity::client_id_for(DataPlaneId(Uuid::from_u128(2)));

        assert_ne!(one, other);
        assert!(one.starts_with("herald-"), "{one}");
        assert!(one.contains(&Uuid::from_u128(1).to_string()));
    }
}
