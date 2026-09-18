/// A TLS certificate and its private key, PEM-encoded -- received from the
/// control plane over a heartbeat and written into this cluster's own
/// Gateway TLS Secret.
///
/// The shape is duplicated from `aether-domain`'s own `Certificate`,
/// deliberately and visibly: this crate does not depend on it, the same way
/// `DeploymentPayloadV1` in `genesis-core` does not depend on `aether-core`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedCertificate {
    pub certificate_pem: String,
    pub private_key_pem: String,

    /// What the next heartbeat reports back, so an unchanged certificate is
    /// not sent again every cycle.
    pub fingerprint: String,
}
