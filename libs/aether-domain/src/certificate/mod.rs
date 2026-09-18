//! Distributing one certificate to every data plane's own Gateway.
//!
//! A data plane's Gateway terminates its own TLS -- there is no central edge
//! that could do it instead -- so every one of them needs a valid certificate
//! for `*.autharie.fr`, without holding the OVH credentials that would let it
//! prove that to Let's Encrypt itself. The control plane issues one wildcard
//! certificate centrally and hands it out instead, over the heartbeat: see
//! [`crate::dns::DnsProvider`] for the DNS-01 challenge this reuses, and
//! `aether-api`'s heartbeat handler for where a certificate actually crosses
//! the wire.
//!
//! [`CertificateSource`] is this platform's own shape for "somewhere the
//! current certificate can be read from" -- a Kubernetes Secret cert-manager
//! already keeps current, in every installation running one today, and
//! whatever else stands behind this port on the day that changes. It only
//! reads: issuing and renewing is somebody else's job, the same way `aether-s3`
//! does not create the bucket it archives into.

use sha2::{Digest, Sha256};
use thiserror::Error;

/// A TLS certificate and its private key, PEM-encoded.
///
/// The exact shape a Kubernetes `kubernetes.io/tls` Secret holds under
/// `tls.crt`/`tls.key` -- both where the control plane reads this from and
/// what a data plane's own `gateway.tls.secretName` already expects to find.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    pub certificate_pem: String,
    pub private_key_pem: String,
}

impl Certificate {
    /// A stable fingerprint of this certificate, so a heartbeat can say
    /// whether the one a data plane already holds is still the current one
    /// without sending the whole thing -- key included -- every cycle.
    ///
    /// Hashes the certificate alone, never the key: the key changes exactly
    /// when the certificate does, so hashing it too would put key material
    /// through a code path that has no reason to see it.
    pub fn fingerprint(&self) -> String {
        let digest = Sha256::digest(self.certificate_pem.as_bytes());
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// What went wrong reading the current certificate.
///
/// One case, not two: unlike [`crate::dns::DnsError`], there is no refusal to
/// tell apart from unavailability -- this port only ever reads, and a source
/// that answers either has the certificate or it does not exist yet.
#[derive(Debug, Clone, Error)]
pub enum CertificateError {
    #[error("the certificate source is unreachable: {reason}")]
    SourceUnavailable { reason: String },

    #[error("the certificate source returned something this platform cannot read: {reason}")]
    Malformed { reason: String },
}

/// Somewhere the control plane's own wildcard certificate can be read from.
///
/// One method: a heartbeat cycle only ever needs to know what is current
/// right now, and issuing or renewing the certificate behind this answer is
/// deliberately not this port's job -- cert-manager's, in every installation
/// running one today.
pub trait CertificateSource: Send + Sync {
    /// The current certificate.
    ///
    /// Absence of a source entirely -- this installation was not configured
    /// with one -- is not represented here: the same way [`crate::dns::DnsProvider`]
    /// is never constructed for an installation with no zone, a caller with
    /// no [`CertificateSource`] to call never reaches this method at all.
    fn current(&self) -> impl Future<Output = Result<Certificate, CertificateError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn certificate(pem: &str) -> Certificate {
        Certificate {
            certificate_pem: pem.to_string(),
            private_key_pem: "irrelevant-to-the-fingerprint".to_string(),
        }
    }

    #[test]
    fn the_same_certificate_fingerprints_the_same_way_twice() {
        let one = certificate("-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----");
        let other = certificate("-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----");

        assert_eq!(one.fingerprint(), other.fingerprint());
    }

    #[test]
    fn a_different_certificate_fingerprints_differently() {
        let one = certificate("-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----");
        let renewed = certificate("-----BEGIN CERTIFICATE-----\nxyz\n-----END CERTIFICATE-----");

        assert_ne!(one.fingerprint(), renewed.fingerprint());
    }

    /// The key never travels through the hash. A leak of the fingerprint
    /// alone -- logged, say -- must not be a leak of anything that unlocks
    /// the private key.
    #[test]
    fn two_certificates_with_the_same_public_half_but_different_keys_fingerprint_the_same() {
        let one = Certificate {
            certificate_pem: "-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----"
                .to_string(),
            private_key_pem: "key-one".to_string(),
        };
        let other = Certificate {
            certificate_pem: "-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----"
                .to_string(),
            private_key_pem: "key-two".to_string(),
        };

        assert_eq!(one.fingerprint(), other.fingerprint());
    }
}
