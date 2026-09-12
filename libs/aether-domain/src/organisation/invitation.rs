//! A promise of membership, addressed to an email rather than to a person.
//!
//! Somebody who has never signed in has no user row, so there is nothing to
//! attach a membership to. An invitation is how they arrive: a secret handed
//! to one address, exchanged once for a place in an organisation.

use std::fmt;

use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{
    CoreError,
    organisation::OrganisationId,
    role::{Role, RoleId},
    user::UserId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub struct InvitationId(pub Uuid);

/// How long a link is good for.
///
/// Long enough to survive a weekend and a forgotten inbox, short enough that
/// a link found in an old message is no longer a way in. Not configurable
/// yet: one number nobody has argued about beats a setting nobody sets.
pub const INVITATION_LIFETIME_DAYS: i64 = 7;

/// The secret in the clear. Handed over once, at creation, and never again.
///
/// No `Debug`, no `Display`, no `Serialize`, and zeroed when it goes out of
/// scope -- the same treatment key material gets, for the same reason. This
/// one is entry into somebody else's organisation.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct InvitationToken(String);

impl InvitationToken {
    /// 32 bytes of randomness, hex encoded.
    ///
    /// High entropy on purpose: it is looked up by hash, so it is never
    /// guessed at a rate a lockout could slow down. Nothing here is derived
    /// from the email, the organisation or the time -- all three are known to
    /// somebody who should not be able to build a link.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);

        let hex = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        bytes.zeroize();

        Self(hex)
    }

    pub fn parse(raw: &str) -> Result<Self, CoreError> {
        let value = raw.trim();

        // Checked for shape before it is hashed, so a request carrying
        // something that could not be a token is refused as malformed rather
        // than looked up and reported as not found -- which would say the
        // same thing about a real token somebody revoked.
        if value.len() != 64 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CoreError::InvitationNotFound);
        }

        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    /// What is stored. A plain SHA-256: the input is 256 bits of randomness,
    /// so there is nothing for a slow hash to protect against -- those exist
    /// for secrets a person chose.
    pub fn hash(&self) -> InvitationTokenHash {
        let digest = Sha256::digest(self.0.as_bytes());

        InvitationTokenHash(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }
}

/// The hash, which is what a row holds and what a lookup matches on.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InvitationTokenHash(String);

impl InvitationTokenHash {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The address an invitation is written to.
///
/// Lowercased, because that is how it is matched against whoever turns up
/// holding the link. Two spellings of one address being treated as two
/// addresses is how an invitation stops working for the person it was for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, ToSchema)]
#[schema(value_type = String, example = "colleague@acme.com")]
pub struct InvitedEmail(String);

impl InvitedEmail {
    pub fn parse(raw: &str) -> Result<Self, CoreError> {
        let value = raw.trim().to_ascii_lowercase();

        // The shape the column enforces, checked here so the refusal names
        // the value rather than arriving as a constraint violation.
        let (local, domain) = value.split_once('@').ok_or_else(|| CoreError::NotAnEmail {
            value: raw.to_string(),
        })?;

        if local.is_empty()
            || domain.is_empty()
            || domain.contains('@')
            || value.chars().any(char::is_whitespace)
        {
            return Err(CoreError::NotAnEmail {
                value: raw.to_string(),
            });
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InvitedEmail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where an invitation stands, now.
///
/// Derived rather than stored. A stored state and a pair of timestamps
/// disagree the moment one is written without the other, and the clock is the
/// thing that decides expiry anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InvitationState {
    Pending,
    Accepted,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Invitation {
    pub id: InvitationId,
    pub organisation_id: OrganisationId,
    pub email: InvitedEmail,

    /// What they will hold once they are in.
    pub roles: Vec<Role>,

    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub invited_by: Option<UserId>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl Invitation {
    /// A fresh invitation and the secret that opens it.
    ///
    /// Returned as a pair rather than stored on the struct: the secret exists
    /// for one response and the row must never be able to carry it.
    pub fn issue(
        organisation_id: OrganisationId,
        email: InvitedEmail,
        roles: Vec<Role>,
        invited_by: Option<UserId>,
        now: DateTime<Utc>,
    ) -> (Self, InvitationToken) {
        let token = InvitationToken::generate();

        let invitation = Self {
            id: InvitationId(Uuid::new_v4()),
            organisation_id,
            email,
            roles,
            expires_at: now + Duration::days(INVITATION_LIFETIME_DAYS),
            created_at: now,
            invited_by,
            accepted_at: None,
            revoked_at: None,
        };

        (invitation, token)
    }

    /// Revoked beats accepted beats expired.
    ///
    /// An invitation somebody already walked through is accepted even if the
    /// clock has since passed its expiry: what happened, happened, and
    /// reporting it as expired would describe a membership that exists as one
    /// that never formed.
    pub fn state(&self, now: DateTime<Utc>) -> InvitationState {
        if self.revoked_at.is_some() {
            return InvitationState::Revoked;
        }
        if self.accepted_at.is_some() {
            return InvitationState::Accepted;
        }
        if now >= self.expires_at {
            return InvitationState::Expired;
        }

        InvitationState::Pending
    }

    pub fn role_ids(&self) -> Vec<RoleId> {
        self.roles.iter().map(|role| role.id).collect()
    }

    /// Whether this invitation may be walked through, and why not if it may
    /// not.
    ///
    /// The refusals are told apart on purpose. "It expired" sends somebody to
    /// ask for another one; "it does not exist" sends them looking for a typo
    /// they did not make.
    pub fn claimable(&self, now: DateTime<Utc>) -> Result<(), CoreError> {
        match self.state(now) {
            InvitationState::Pending => Ok(()),
            InvitationState::Accepted => Err(CoreError::InvitationAlreadyAccepted),
            InvitationState::Revoked => Err(CoreError::InvitationRevoked),
            InvitationState::Expired => Err(CoreError::InvitationExpired {
                expired_at: self.expires_at,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn email() -> InvitedEmail {
        InvitedEmail::parse("Colleague@Acme.COM").expect("an address")
    }

    fn invitation(now: DateTime<Utc>) -> Invitation {
        Invitation::issue(
            OrganisationId(Uuid::from_u128(1)),
            email(),
            Vec::new(),
            None,
            now,
        )
        .0
    }

    /// Two spellings of one address are one address. Anything else is an
    /// invitation that stops working for the person it was written for.
    #[test]
    fn an_address_is_kept_in_one_spelling() {
        assert_eq!(email().as_str(), "colleague@acme.com");
        assert_eq!(
            InvitedEmail::parse("  colleague@acme.com ").expect("trimmed"),
            email()
        );
    }

    #[test]
    fn what_is_not_an_address_is_refused() {
        for raw in [
            "",
            "nobody",
            "@acme.com",
            "colleague@",
            "a@b@c",
            "a b@acme.com",
        ] {
            assert!(InvitedEmail::parse(raw).is_err(), "'{raw}' was accepted");
        }
    }

    /// The secret is 256 bits and never derived from anything a stranger
    /// knows. Two invitations issued in the same breath must not collide.
    #[test]
    fn every_token_is_its_own() {
        let one = InvitationToken::generate();
        let two = InvitationToken::generate();

        assert_eq!(one.expose().len(), 64);
        assert_ne!(one.expose(), two.expose());
        assert_ne!(one.hash(), two.hash());
    }

    #[test]
    fn a_token_hashes_the_same_way_every_time() {
        let token = InvitationToken::parse(&"a".repeat(64)).expect("a token");

        assert_eq!(token.hash(), token.hash());
        assert_ne!(token.hash().as_str(), token.expose());
    }

    /// A round trip through the link: what the inviter was shown parses back
    /// to something that hashes to what was stored.
    #[test]
    fn the_secret_shown_once_still_matches_the_row() {
        let token = InvitationToken::generate();
        let stored = token.hash();

        let presented = InvitationToken::parse(token.expose()).expect("a token");

        assert_eq!(presented.hash(), stored);
    }

    /// Refused as malformed rather than looked up. Reporting "not found"
    /// for a string that could not be a token says the same thing as
    /// reporting it for one somebody revoked.
    #[test]
    fn something_that_could_not_be_a_token_is_refused_before_any_lookup() {
        for raw in ["", "short", &"z".repeat(64), &"a".repeat(63)] {
            assert!(InvitationToken::parse(raw).is_err(), "'{raw}' was accepted");
        }
    }

    #[test]
    fn a_fresh_invitation_is_pending_and_expires_a_week_out() {
        let now = Utc::now();
        let invitation = invitation(now);

        assert_eq!(invitation.state(now), InvitationState::Pending);
        assert!(invitation.claimable(now).is_ok());
        assert_eq!(
            invitation.expires_at,
            now + Duration::days(INVITATION_LIFETIME_DAYS)
        );
    }

    /// Says it expired. "It does not exist" would send somebody looking for a
    /// typo they did not make, rather than asking for another link.
    #[test]
    fn an_expired_invitation_says_so() {
        let now = Utc::now();
        let invitation = invitation(now - Duration::days(INVITATION_LIFETIME_DAYS + 1));

        assert_eq!(invitation.state(now), InvitationState::Expired);
        assert!(matches!(
            invitation.claimable(now),
            Err(CoreError::InvitationExpired { .. })
        ));
    }

    #[test]
    fn an_invitation_cannot_be_walked_through_twice() {
        let now = Utc::now();
        let mut invitation = invitation(now);
        invitation.accepted_at = Some(now);

        assert_eq!(invitation.state(now), InvitationState::Accepted);
        assert!(matches!(
            invitation.claimable(now),
            Err(CoreError::InvitationAlreadyAccepted)
        ));
    }

    #[test]
    fn a_revoked_invitation_is_refused_as_revoked() {
        let now = Utc::now();
        let mut invitation = invitation(now);
        invitation.revoked_at = Some(now);

        assert!(matches!(
            invitation.claimable(now),
            Err(CoreError::InvitationRevoked)
        ));
    }

    /// What happened, happened. An invitation somebody already walked through
    /// stays accepted once the clock passes its expiry, because reporting it
    /// as expired would describe a membership that exists as one that never
    /// formed.
    #[test]
    fn acceptance_outlives_the_expiry_it_beat() {
        let issued = Utc::now() - Duration::days(INVITATION_LIFETIME_DAYS + 1);
        let mut invitation = invitation(issued);
        invitation.accepted_at = Some(issued);

        assert_eq!(invitation.state(Utc::now()), InvitationState::Accepted);
    }

    /// And revoking beats both, so a link cut off after somebody used it
    /// still reads as cut off.
    #[test]
    fn revoking_shows_even_on_one_already_accepted() {
        let now = Utc::now();
        let mut invitation = invitation(now);
        invitation.accepted_at = Some(now);
        invitation.revoked_at = Some(now);

        assert_eq!(invitation.state(now), InvitationState::Revoked);
    }
}
