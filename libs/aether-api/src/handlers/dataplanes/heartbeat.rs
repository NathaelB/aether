use aether_auth::Identity;
use aether_core::{
    dataplane::{ports::DataPlaneService, value_objects::DataPlaneId},
    version::Version,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    certificate::certificate_for_heartbeat, errors::ApiError, response::Response, state::AppState,
};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/heartbeat")]
pub struct HeartbeatRoute {
    pub dataplane_id: DataPlaneId,
}

/// Empty body accepted, since Herald builds that predate this feature send
/// none: `#[serde(default)]` on the only field means an absent body still
/// parses, rather than every existing Herald failing a heartbeat overnight.
#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct HeartbeatRequest {
    /// The operator/chart version this Herald is running. Absent means it is
    /// not reported this cycle -- the control plane keeps whatever it last
    /// recorded rather than treating silence as a downgrade.
    #[serde(default)]
    pub operator_version: Option<String>,

    /// Where this data plane's own Gateway answers, read back from its
    /// LoadBalancer address. Absent the same way `operator_version` can be --
    /// an older Herald, or a cycle where reading it failed -- keeps whatever
    /// was last recorded rather than clearing it.
    #[serde(default)]
    pub gateway_address: Option<String>,

    /// The fingerprint of the certificate this data plane's own Gateway is
    /// currently serving, if it has one. Absent means this Herald has never
    /// received one, or does not report one yet -- either way, the response
    /// sends the current certificate rather than assuming it is already
    /// there.
    #[serde(default)]
    pub certificate_fingerprint: Option<String>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct HeartbeatResponseData {
    /// `false` when no data plane carries this id, so a Herald configured with
    /// a stale one learns it instead of reporting into the void.
    pub recorded: bool,

    /// The certificate this data plane's own Gateway should be serving,
    /// present only when it differs from what the request said Herald
    /// already has. Absent means exactly that: nothing changed, or this
    /// installation distributes no certificate at all -- either way, there
    /// is nothing new to write.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<CertificatePayload>,
}

/// A certificate crossing the wire, PEM-encoded -- the same shape a
/// Kubernetes `kubernetes.io/tls` Secret holds it in, on both ends of this
/// trip.
#[derive(Serialize, ToSchema, PartialEq)]
pub struct CertificatePayload {
    pub certificate_pem: String,
    pub private_key_pem: String,

    /// What the next heartbeat should report back, so this is not resent
    /// every cycle once Herald has written it.
    pub fingerprint: String,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct HeartbeatResponse {
    data: HeartbeatResponseData,
}

#[utoipa::path(
    post,
    path = "/{dataplane_id}/heartbeat",
    summary = "report that a data plane is alive",
    tag = "dataplanes",
    description = "Records that this data plane's Herald is running, and which operator/chart \
                   version it reports. A data plane that stops reporting is no longer selected \
                   for new deployments, and one whose reported version is too old holds back \
                   releases that require a newer operator.",
    params(HeartbeatRoute),
    request_body = HeartbeatRequest,
    responses(
        (status = 200, description = "Heartbeat recorded", body = HeartbeatResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 400, description = "Invalid dataplane id or operator version", body = ApiError),
        (status = 403, description = "Caller is not herald", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn heartbeat_handler(
    HeartbeatRoute { dataplane_id }: HeartbeatRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    body: Option<Json<HeartbeatRequest>>,
) -> Result<Response<HeartbeatResponse>, ApiError> {
    let request = body.map(|Json(request)| request).unwrap_or_default();

    let operator_version = request
        .operator_version
        .map(|raw| {
            Version::parse(&raw).map_err(|e| ApiError::BadRequest {
                reason: e.to_string(),
            })
        })
        .transpose()?;

    let recorded = state
        .service
        .record_heartbeat(
            identity,
            dataplane_id,
            operator_version,
            request.gateway_address,
        )
        .await?;

    // Read after the transaction closes rather than inside it: this is a
    // Kubernetes API call, not a database one, and a slow cluster has no
    // reason to hold a Postgres transaction open while it answers.
    let certificate = certificate_for_heartbeat(
        state.certificate_source.as_deref(),
        request.certificate_fingerprint.as_deref(),
    )
    .await
    .map(|certificate| CertificatePayload {
        fingerprint: certificate.fingerprint(),
        certificate_pem: certificate.certificate_pem,
        private_key_pem: certificate.private_key_pem,
    });

    Ok(Response::OK(HeartbeatResponse {
        data: HeartbeatResponseData {
            recorded,
            certificate,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::app_state;
    use aether_auth::Client;
    use uuid::Uuid;

    fn non_herald_identity() -> Identity {
        Identity::Client(Client {
            id: "id".to_string(),
            client_id: "some-other-service".to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

    /// A forged heartbeat would keep a dead cluster receiving deployments, so
    /// the endpoint carries the same rule as claim and ack. The rule itself is
    /// asserted on the domain service; here the transaction opens first, so an
    /// unreachable database is what surfaces.
    #[tokio::test]
    async fn heartbeat_requires_a_service_identity() {
        let result = heartbeat_handler(
            HeartbeatRoute {
                dataplane_id: DataPlaneId(Uuid::new_v4()),
            },
            State(app_state()),
            Extension(non_herald_identity()),
            None,
        )
        .await;

        assert!(result.is_err());
    }
}
