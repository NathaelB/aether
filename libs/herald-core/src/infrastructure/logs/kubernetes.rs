use std::time::Duration;

use chrono::{DateTime, Utc};
use futures::{AsyncBufReadExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::Client;
use kube::api::{Api, ListParams, LogParams};
use tokio::sync::mpsc::{self, Sender};
use tracing::{debug, warn};

use crate::domain::entities::deployment::DeploymentKind;
use crate::domain::entities::logs::{LogLine, LogStreamRequest};
use crate::domain::error::HeraldError;
use crate::domain::ports::PodLogSource;

/// How many lines may wait for the session loop before the read inside the
/// cluster is slowed down.
///
/// Bounded on purpose. An instance producing faster than the control plane
/// accepts must push back on the read rather than fill this process with a
/// customer's logs -- which are also the one thing this feature promises not
/// to accumulate anywhere.
const BUFFER: usize = 1_024;

/// How long listing a deployment's pods may take before it counts as
/// unreadable.
///
/// A session that hangs here shows the reader an empty screen and never says
/// why. Giving up ends it cleanly instead.
const LIST_TIMEOUT: Duration = Duration::from_secs(10);

/// Reads pod logs through the Kubernetes API of the cluster Herald runs in.
pub struct KubePodLogSource {
    client: Client,
}

impl KubePodLogSource {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Builds a client from the ambient configuration: the service account
    /// when running as a pod, the local kubeconfig otherwise.
    pub async fn from_env() -> Result<Self, HeraldError> {
        let client = Client::try_default()
            .await
            .map_err(|error| HeraldError::Internal {
                message: format!("failed to build a Kubernetes client: {error}"),
            })?;

        Ok(Self::new(client))
    }
}

/// The pods that belong to one deployment, as the operator labels them.
///
/// Selected rather than "everything in the namespace": the namespace also
/// holds the deployment's database, and somebody who asked for their identity
/// provider's logs did not ask for Postgres's.
fn selector(request: &LogStreamRequest) -> String {
    let product = match request.kind {
        DeploymentKind::Ferriskey => "ferriskey",
        DeploymentKind::Keycloak => "keycloak",
    };

    format!(
        "app.kubernetes.io/name={product},app.kubernetes.io/instance=deployment-{}",
        request.deployment_id
    )
}

/// Splits the RFC3339 stamp Kubernetes prepends when `timestamps` is on.
///
/// A line whose stamp cannot be read keeps its whole text and is stamped with
/// now. Dropping it would be worse: it is still something the instance said,
/// and a log view that silently omits lines is a log view that lies.
fn split_timestamp(raw: &str) -> (DateTime<Utc>, String) {
    match raw.split_once(' ') {
        Some((stamp, rest)) => match DateTime::parse_from_rfc3339(stamp) {
            Ok(at) => (at.with_timezone(&Utc), rest.to_string()),
            Err(_) => (Utc::now(), raw.to_string()),
        },
        None => (Utc::now(), raw.to_string()),
    }
}

/// Follows one container until it stops, the reader goes away, or it fails.
async fn follow_container(
    pods: Api<Pod>,
    pod: String,
    container: String,
    since_seconds: i64,
    lines: Sender<LogLine>,
) {
    let params = LogParams {
        container: Some(container.clone()),
        follow: true,
        since_seconds: Some(since_seconds),
        timestamps: true,
        ..Default::default()
    };

    let stream = match pods.log_stream(&pod, &params).await {
        Ok(stream) => stream,
        Err(error) => {
            warn!(%pod, %container, %error, "could not open a log stream");
            return;
        }
    };

    let mut reader = stream.lines();

    loop {
        match reader.try_next().await {
            Ok(Some(raw)) => {
                let (at, message) = split_timestamp(&raw);

                // An error here means the session ended and nobody is holding
                // the other end any more, which is also the signal to stop
                // reading from the cluster.
                if lines
                    .send(LogLine {
                        at,
                        source: container.clone(),
                        message,
                    })
                    .await
                    .is_err()
                {
                    debug!(%pod, %container, "the log session ended; stopping the read");
                    return;
                }
            }
            Ok(None) => return,
            Err(error) => {
                warn!(%pod, %container, %error, "a log stream ended early");
                return;
            }
        }
    }
}

impl PodLogSource for KubePodLogSource {
    async fn follow(
        &self,
        request: &LogStreamRequest,
    ) -> Result<mpsc::Receiver<LogLine>, HeraldError> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &request.namespace);
        let params = ListParams::default().labels(&selector(request));

        let listed = tokio::time::timeout(LIST_TIMEOUT, pods.list(&params))
            .await
            .map_err(|_| HeraldError::Internal {
                message: format!("listing the pods of {} timed out", request.deployment_id),
            })?
            .map_err(|error| HeraldError::Internal {
                message: format!(
                    "could not list the pods of {}: {error}",
                    request.deployment_id
                ),
            })?;

        let containers: Vec<(String, String)> = listed
            .items
            .iter()
            .filter_map(|pod| {
                let name = pod.metadata.name.clone()?;
                let containers = pod.spec.as_ref()?.containers.iter();

                Some(
                    containers
                        .map(|container| (name.clone(), container.name.clone()))
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
            .collect();

        // Nothing to read is a failure, not an empty stream. An empty stream
        // would leave somebody watching a blank screen with no way to tell it
        // apart from an instance that happens to be silent.
        if containers.is_empty() {
            return Err(HeraldError::Internal {
                message: format!(
                    "no pods matching {} in {}",
                    selector(request),
                    request.namespace
                ),
            });
        }

        let (sender, receiver) = mpsc::channel(BUFFER);
        let since_seconds = i64::from(request.since_minutes) * 60;

        for (pod, container) in containers {
            tokio::spawn(follow_container(
                pods.clone(),
                pod,
                container,
                since_seconds,
                sender.clone(),
            ));
        }

        Ok(receiver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::dataplane::DataPlaneId;
    use crate::domain::entities::deployment::DeploymentId;
    use crate::domain::entities::logs::LogSessionId;
    use uuid::Uuid;

    fn request(kind: DeploymentKind) -> LogStreamRequest {
        LogStreamRequest {
            deployment_id: DeploymentId::new("22222222-2222-2222-2222-222222222222"),
            dataplane_id: DataPlaneId::new(Uuid::nil().to_string()),
            namespace: "aether-acme".to_string(),
            kind,
            session_id: LogSessionId(Uuid::nil()),
            since_minutes: 5,
        }
    }

    /// The labels are the operator's, and getting them wrong means reading
    /// nothing or reading the database's logs instead.
    #[test]
    fn the_selector_matches_the_labels_the_operator_writes() {
        assert_eq!(
            selector(&request(DeploymentKind::Ferriskey)),
            "app.kubernetes.io/name=ferriskey,\
             app.kubernetes.io/instance=deployment-22222222-2222-2222-2222-222222222222"
        );
        assert_eq!(
            selector(&request(DeploymentKind::Keycloak)),
            "app.kubernetes.io/name=keycloak,\
             app.kubernetes.io/instance=deployment-22222222-2222-2222-2222-222222222222"
        );
    }

    #[test]
    fn a_stamped_line_keeps_its_own_time_and_loses_the_stamp() {
        let (at, message) = split_timestamp("2026-09-11T10:00:00.123456789Z started in 4.2s");

        assert_eq!(at.to_rfc3339(), "2026-09-11T10:00:00.123456789+00:00");
        assert_eq!(message, "started in 4.2s");
    }

    /// A line whose stamp will not parse is still something the instance
    /// said. Dropping it would make the view quietly incomplete.
    #[test]
    fn a_line_with_no_readable_stamp_keeps_all_of_its_text() {
        let (_, message) = split_timestamp("not-a-timestamp and the rest of the line");

        assert_eq!(message, "not-a-timestamp and the rest of the line");
    }

    #[test]
    fn a_line_with_no_space_at_all_keeps_all_of_its_text() {
        let (_, message) = split_timestamp("singleword");

        assert_eq!(message, "singleword");
    }
}
