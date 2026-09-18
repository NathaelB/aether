//! Where this data plane's own Gateway answers.
//!
//! Read once, best-effort, the same way `operator_version` is decided at
//! startup rather than re-derived every cycle: a Gateway's LoadBalancer
//! address does not change under a running cluster, and a Herald that could
//! not read it once is not going to succeed by retrying on every heartbeat.

use kube::Client;
use kube::api::Api;
use kube::core::{ApiResource, DynamicObject, GroupVersionKind};
use tracing::warn;

const GROUP: &str = "gateway.networking.k8s.io";
const VERSION: &str = "v1";

fn gateway_api_resource() -> ApiResource {
    ApiResource::from_gvk(&GroupVersionKind::gvk(GROUP, VERSION, "Gateway"))
}

/// Reads this data plane's own Gateway address from its name and namespace,
/// building the Kubernetes client itself.
///
/// Both are optional, unlike the operator's `Edge::from_env`: a chart not
/// yet updated to set them, or a data plane with no Gateway of its own, is
/// not a Herald that fails to start -- it is one that never learns a
/// `gateway_address` to report, same as an older Herald binary would.
pub async fn resolve(
    gateway_name: Option<&str>,
    gateway_namespace: Option<&str>,
) -> Option<String> {
    let (gateway_name, gateway_namespace) = match (gateway_name, gateway_namespace) {
        (Some(name), Some(namespace)) => (name, namespace),
        _ => return None,
    };

    let client = match Client::try_default().await {
        Ok(client) => client,
        Err(error) => {
            warn!(%error, "could not build a Kubernetes client to read this data plane's own Gateway");
            return None;
        }
    };

    read_gateway_address(client, gateway_name, gateway_namespace).await
}

/// The first address in a Gateway's `.status.addresses`.
///
/// A Gateway can carry more than one; the first is what every data plane
/// here has exactly one of, a single LoadBalancer IP or hostname. Absent
/// entirely -- the Gateway not found, not yet assigned an address, or
/// unreadable -- is not an error a caller needs to act on: it is exactly the
/// case a heartbeat with no `gateway_address` already handles.
pub async fn read_gateway_address(
    client: Client,
    gateway_name: &str,
    gateway_namespace: &str,
) -> Option<String> {
    let gateways: Api<DynamicObject> =
        Api::namespaced_with(client, gateway_namespace, &gateway_api_resource());

    let gateway = match gateways.get(gateway_name).await {
        Ok(gateway) => gateway,
        Err(error) => {
            warn!(
                gateway_name,
                gateway_namespace,
                %error,
                "could not read this data plane's own Gateway"
            );
            return None;
        }
    };

    let address = first_address(&gateway);

    if address.is_none() {
        warn!(
            gateway_name,
            gateway_namespace, "this data plane's own Gateway has no address yet"
        );
    }

    address
}

/// Pulled out of [`read_gateway_address`] so the parsing can be tested
/// without a cluster to read from.
fn first_address(gateway: &DynamicObject) -> Option<String> {
    gateway
        .data
        .get("status")
        .and_then(|status| status.get("addresses"))
        .and_then(|addresses| addresses.as_array())
        .and_then(|addresses| addresses.first())
        .and_then(|address| address.get("value"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn gateway(status: serde_json::Value) -> DynamicObject {
        DynamicObject {
            types: None,
            metadata: Default::default(),
            data: json!({ "status": status }),
        }
    }

    #[test]
    fn the_first_address_is_read() {
        let gateway = gateway(json!({
            "addresses": [
                { "type": "IPAddress", "value": "203.0.113.10" },
                { "type": "IPAddress", "value": "203.0.113.11" }
            ]
        }));

        assert_eq!(first_address(&gateway).as_deref(), Some("203.0.113.10"));
    }

    #[test]
    fn a_gateway_with_no_address_yet_reads_as_none() {
        assert_eq!(first_address(&gateway(json!({ "addresses": [] }))), None);
        assert_eq!(first_address(&gateway(json!({}))), None);
    }
}
