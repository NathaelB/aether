//! Reading the control plane's own wildcard certificate from a Kubernetes
//! Secret -- cert-manager's, in every installation running one today.
//!
//! The control plane runs in the same cluster as the Gateway it fronts
//! `console`/`api` with, and that Gateway's own certificate is already kept
//! current there by cert-manager against the same zone `aether-ovh` issues
//! DNS records in. This reads it back rather than requesting one of its own:
//! issuing a second certificate for the same wildcard would be a second
//! renewal clock to keep in sync with the first, for no reader that needs
//! anything the first does not already provide.

use aether_core::certificate::{Certificate, CertificateError, CertificateSource};
use k8s_openapi::api::core::v1::Secret;
use kube::Client;
use kube::api::Api;
use tracing::warn;

/// The keys a Kubernetes `kubernetes.io/tls` Secret always carries.
const CERTIFICATE_KEY: &str = "tls.crt";
const PRIVATE_KEY: &str = "tls.key";

pub struct KubeCertificateSource {
    client: Client,
    secret_name: String,
    secret_namespace: String,
}

impl KubeCertificateSource {
    pub fn new(client: Client, secret_name: String, secret_namespace: String) -> Self {
        Self {
            client,
            secret_name,
            secret_namespace,
        }
    }

    /// Builds a client from the ambient configuration: the service account
    /// when running as a pod, the local kubeconfig otherwise. Matches every
    /// other Kubernetes client built this way across the workspace.
    pub async fn from_env(
        secret_name: String,
        secret_namespace: String,
    ) -> Result<Self, CertificateError> {
        let client =
            Client::try_default()
                .await
                .map_err(|error| CertificateError::SourceUnavailable {
                    reason: format!("failed to build a Kubernetes client: {error}"),
                })?;

        Ok(Self::new(client, secret_name, secret_namespace))
    }

    fn read(secret: &Secret, key: &str) -> Result<String, CertificateError> {
        let data = secret
            .data
            .as_ref()
            .ok_or_else(|| CertificateError::Malformed {
                reason: format!("the secret has no data, missing `{key}`"),
            })?;

        let bytes = data.get(key).ok_or_else(|| CertificateError::Malformed {
            reason: format!("missing `{key}`"),
        })?;

        String::from_utf8(bytes.0.clone()).map_err(|error| CertificateError::Malformed {
            reason: format!("`{key}` is not valid UTF-8: {error}"),
        })
    }
}

impl CertificateSource for KubeCertificateSource {
    async fn current(&self) -> Result<Certificate, CertificateError> {
        let secrets: Api<Secret> = Api::namespaced(self.client.clone(), &self.secret_namespace);

        let secret = secrets.get(&self.secret_name).await.map_err(|error| {
            CertificateError::SourceUnavailable {
                reason: format!(
                    "could not read {}/{}: {error}",
                    self.secret_namespace, self.secret_name
                ),
            }
        })?;

        Ok(Certificate {
            certificate_pem: Self::read(&secret, CERTIFICATE_KEY)?,
            private_key_pem: Self::read(&secret, PRIVATE_KEY)?,
        })
    }
}

/// What a heartbeat's response should carry about the certificate, given
/// what its request said a data plane already holds.
///
/// `None` for either of two different reasons that look the same to a
/// heartbeat: this installation distributes no certificate at all, or the
/// data plane already has the current one and there is nothing to resend. A
/// source that cannot be read is also `None` -- logged, not surfaced -- the
/// same way a heartbeat never fails over `gateway_address` being unreadable:
/// this is a cycle that catches up, not a request that failed.
///
/// Generic over the source rather than naming [`KubeCertificateSource`]
/// directly, so the decision below -- has this changed since the data plane
/// last saw it -- can be tested against a source that needs no cluster to
/// read from.
pub async fn certificate_for_heartbeat<S: CertificateSource>(
    source: Option<&S>,
    known_fingerprint: Option<&str>,
) -> Option<Certificate> {
    let source = source?;

    let certificate = match source.current().await {
        Ok(certificate) => certificate,
        Err(error) => {
            warn!(%error, "could not read the current certificate; nothing to send this cycle");
            return None;
        }
    };

    (known_fingerprint != Some(certificate.fingerprint().as_str())).then_some(certificate)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedSource(Certificate);

    impl CertificateSource for FixedSource {
        async fn current(&self) -> Result<Certificate, CertificateError> {
            Ok(self.0.clone())
        }
    }

    struct FailingSource;

    impl CertificateSource for FailingSource {
        async fn current(&self) -> Result<Certificate, CertificateError> {
            Err(CertificateError::SourceUnavailable {
                reason: "unreachable in this test".to_string(),
            })
        }
    }

    fn certificate() -> Certificate {
        Certificate {
            certificate_pem: "-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----"
                .to_string(),
            private_key_pem: "-----BEGIN PRIVATE KEY-----\nsecret\n-----END PRIVATE KEY-----"
                .to_string(),
        }
    }

    #[tokio::test]
    async fn no_source_configured_sends_nothing() {
        let sent = certificate_for_heartbeat(None::<&FixedSource>, None).await;

        assert!(sent.is_none());
    }

    /// The point of the fingerprint travelling both ways: a data plane that
    /// already has the current certificate is not sent it again.
    #[tokio::test]
    async fn a_data_plane_that_already_has_the_current_certificate_gets_nothing() {
        let certificate = certificate();
        let source = FixedSource(certificate.clone());

        let sent = certificate_for_heartbeat(Some(&source), Some(&certificate.fingerprint())).await;

        assert!(sent.is_none());
    }

    #[tokio::test]
    async fn a_data_plane_with_no_certificate_yet_receives_the_current_one() {
        let certificate = certificate();
        let source = FixedSource(certificate.clone());

        let sent = certificate_for_heartbeat(Some(&source), None).await;

        assert_eq!(sent, Some(certificate));
    }

    /// A renewed certificate has a new fingerprint, so it does not match
    /// what the data plane last reported, and travels on the next heartbeat.
    #[tokio::test]
    async fn a_data_plane_with_a_stale_fingerprint_receives_the_renewed_certificate() {
        let certificate = certificate();
        let source = FixedSource(certificate.clone());

        let sent =
            certificate_for_heartbeat(Some(&source), Some("a-fingerprint-from-before-renewal"))
                .await;

        assert_eq!(sent, Some(certificate));
    }

    /// The source could not be read this cycle. Nothing is sent, and nothing
    /// about the heartbeat itself fails over it.
    #[tokio::test]
    async fn a_source_that_cannot_be_read_sends_nothing() {
        let sent = certificate_for_heartbeat(Some(&FailingSource), None).await;

        assert!(sent.is_none());
    }
}
