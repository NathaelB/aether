//! Writing this cluster's own Gateway TLS Secret.
//!
//! `charts/aether-dataplane`'s `gateway.tls.secretName` already expects
//! exactly this shape -- a `kubernetes.io/tls` Secret in the Gateway's own
//! namespace -- because it was written for bring-your-own-secret from the
//! start. This is what keeps that secret's contents current when an
//! installation opts into a managed one instead: the chart itself does not
//! change.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use k8s_openapi::api::core::v1::Secret;
use kube::Client;
use kube::api::{Api, ObjectMeta, Patch, PatchParams};

use crate::domain::entities::certificate::ReceivedCertificate;
use crate::domain::error::HeraldError;
use crate::domain::ports::GatewayCertificateSink;

/// The field manager server-side apply writes under. Named after this
/// process rather than the operator's: the two could apply to the same
/// namespace one day, and a shared name would have either silently take
/// fields back from the other.
const FIELD_MANAGER: &str = "herald";

pub struct KubeGatewayCertificateSink {
    client: Client,
    secret_name: String,
    namespace: String,
}

impl KubeGatewayCertificateSink {
    pub fn new(client: Client, secret_name: String, namespace: String) -> Self {
        Self {
            client,
            secret_name,
            namespace,
        }
    }

    /// Builds a client from the ambient configuration: the service account
    /// when running as a pod, the local kubeconfig otherwise. Matches every
    /// other Kubernetes client built this way across the workspace.
    pub async fn from_env(secret_name: String, namespace: String) -> Result<Self, HeraldError> {
        let client = Client::try_default()
            .await
            .map_err(|error| HeraldError::Internal {
                message: format!("failed to build a Kubernetes client: {error}"),
            })?;

        Ok(Self::new(client, secret_name, namespace))
    }
}

impl GatewayCertificateSink for KubeGatewayCertificateSink {
    fn write<'a>(
        &'a self,
        certificate: ReceivedCertificate,
    ) -> Pin<Box<dyn Future<Output = Result<(), HeraldError>> + Send + 'a>> {
        Box::pin(async move {
            let secrets: Api<Secret> = Api::namespaced(self.client.clone(), &self.namespace);

            let mut string_data = BTreeMap::new();
            string_data.insert("tls.crt".to_string(), certificate.certificate_pem);
            string_data.insert("tls.key".to_string(), certificate.private_key_pem);

            let secret = Secret {
                metadata: ObjectMeta {
                    name: Some(self.secret_name.clone()),
                    namespace: Some(self.namespace.clone()),
                    ..Default::default()
                },
                type_: Some("kubernetes.io/tls".to_string()),
                string_data: Some(string_data),
                ..Default::default()
            };

            // Server-side apply, the same way the operator writes a Secret it
            // owns: idempotent, and it creates the Secret on the first call
            // rather than needing a separate check for whether it exists yet.
            secrets
                .patch(
                    &self.secret_name,
                    &PatchParams::apply(FIELD_MANAGER).force(),
                    &Patch::Apply(&secret),
                )
                .await
                .map_err(|error| HeraldError::Internal {
                    message: format!(
                        "could not write {}/{}: {error}",
                        self.namespace, self.secret_name
                    ),
                })?;

            Ok(())
        })
    }
}
