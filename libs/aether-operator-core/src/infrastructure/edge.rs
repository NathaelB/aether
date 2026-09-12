//! Where a managed instance meets the outside world.
//!
//! One `HTTPRoute` per instance, attached to the Gateway the data plane chart
//! declares. The Gateway is shared and the operator never touches it: it owns
//! the routes that hang off it and nothing above them.
//!
//! The pieces that decide what gets written, and whether what was written
//! works, are plain functions over JSON. A cluster is not needed to find out
//! that a route points at the wrong port.

use std::collections::BTreeMap;

use aether_crds::v1alpha::identity_instance::IdentityInstance;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::core::{ApiResource, GroupVersionKind};
use serde_json::{Value, json};

use crate::domain::OperatorError;

const GROUP: &str = "gateway.networking.k8s.io";
const VERSION: &str = "v1";

pub fn httproute_api_resource() -> ApiResource {
    ApiResource::from_gvk(&GroupVersionKind::gvk(GROUP, VERSION, "HTTPRoute"))
}

/// The Gateway a tenant route attaches to.
///
/// Passed in from the environment rather than discovered: an operator that
/// picks whichever Gateway it can see would silently move every instance the
/// day a second one appears in the cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub gateway_name: String,
    pub gateway_namespace: String,
}

impl Edge {
    pub fn new(gateway_name: impl Into<String>, gateway_namespace: impl Into<String>) -> Self {
        Self {
            gateway_name: gateway_name.into(),
            gateway_namespace: gateway_namespace.into(),
        }
    }

    /// Reads the Gateway to attach to from the environment.
    ///
    /// Both names are required. Defaulting either one would attach every
    /// route in the cluster to a Gateway nobody chose, and the failure would
    /// show up as traffic going somewhere unexpected rather than as an error.
    pub fn from_env() -> Result<Self, OperatorError> {
        let name = required("AETHER_GATEWAY_NAME")?;
        let namespace = required("AETHER_GATEWAY_NAMESPACE")?;

        Ok(Self::new(name, namespace))
    }
}

fn required(key: &str) -> Result<String, OperatorError> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| OperatorError::Configuration {
            message: format!("{key} is required: it names the Gateway tenant routes attach to"),
        })
}

/// One backend a route sends matching traffic to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backend {
    /// The path prefix this backend answers on.
    pub prefix: String,
    pub service: String,
    pub port: i32,
}

impl Backend {
    pub fn new(prefix: impl Into<String>, service: impl Into<String>, port: i32) -> Self {
        Self {
            prefix: prefix.into(),
            service: service.into(),
            port,
        }
    }
}

/// Builds the route for an instance.
///
/// Rules are written in the order given, but the order does not decide which
/// one wins. Gateway API matches the longest path prefix first, so `/api`
/// beats `/` wherever both could match. That is a guarantee of the spec, not
/// of this list, which is why the backends can be passed in any order.
pub fn build_route(
    instance: &IdentityInstance,
    name: &str,
    namespace: &str,
    labels: &BTreeMap<String, String>,
    owner_reference: Option<OwnerReference>,
    edge: &Edge,
    backends: &[Backend],
) -> Value {
    let rules: Vec<Value> = backends
        .iter()
        .map(|backend| {
            json!({
                "matches": [{ "path": { "type": "PathPrefix", "value": backend.prefix } }],
                "backendRefs": [{ "name": backend.service, "port": backend.port }],
            })
        })
        .collect();

    json!({
        "apiVersion": format!("{GROUP}/{VERSION}"),
        "kind": "HTTPRoute",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": labels,
            "ownerReferences": owner_reference.map(|owner| vec![owner]),
        },
        "spec": {
            // The Gateway lives in the data plane's namespace and the route in
            // the tenant's. That crosses a namespace boundary, which the
            // Gateway permits through allowedRoutes rather than the route
            // asserting it here.
            "parentRefs": [{
                "group": GROUP,
                "kind": "Gateway",
                "name": edge.gateway_name,
                "namespace": edge.gateway_namespace,
            }],
            "hostnames": [instance.spec.hostname],
            "rules": rules,
        }
    })
}

/// Whether the edge is actually serving this instance.
///
/// Both conditions are needed and they fail for different reasons. `Accepted`
/// false means the Gateway refused the route -- a hostname outside its
/// listener, a parent that does not exist. `ResolvedRefs` false means it took
/// the route and cannot reach what the route points at, which is the case the
/// old check could not see at all: it asked whether the object existed, and an
/// object pointing at a service that was never created exists perfectly well.
pub fn route_is_ready(route_status: Option<&Value>) -> bool {
    let Some(parents) = route_status
        .and_then(|status| status.get("parents"))
        .and_then(|parents| parents.as_array())
    else {
        return false;
    };

    parents
        .iter()
        .any(|parent| condition_true(parent, "Accepted") && condition_true(parent, "ResolvedRefs"))
}

fn condition_true(parent: &Value, wanted: &str) -> bool {
    parent
        .get("conditions")
        .and_then(|conditions| conditions.as_array())
        .map(|conditions| {
            conditions.iter().any(|condition| {
                condition.get("type").and_then(Value::as_str) == Some(wanted)
                    && condition.get("status").and_then(Value::as_str) == Some("True")
            })
        })
        .unwrap_or(false)
}

/// Whether this instance is meant to be reachable from outside at all.
pub fn exposed(instance: &IdentityInstance) -> bool {
    instance
        .spec
        .ingress
        .as_ref()
        .map(|exposure| exposure.enabled)
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_crds::common::types::ResourceRequirements;
    use aether_crds::v1alpha::identity_instance::{
        DatabaseConfig, DatabaseMode, IdentityInstanceSpec, IdentityProvider, IngressConfig,
        ManagedClusterConfig, ManagedClusterStorage,
    };
    use kube::core::ObjectMeta;

    fn instance(hostname: &str) -> IdentityInstance {
        IdentityInstance {
            metadata: ObjectMeta {
                name: Some("deployment-1".to_string()),
                namespace: Some("tenant-a".to_string()),
                ..Default::default()
            },
            spec: IdentityInstanceSpec {
                organisation_id: "org".to_string(),
                provider: IdentityProvider::Ferriskey,
                version: "0.5.0".to_string(),
                hostname: hostname.to_string(),
                database: DatabaseConfig {
                    mode: DatabaseMode::ManagedCluster,
                    managed_cluster: ManagedClusterConfig {
                        instances: 1,
                        storage: ManagedClusterStorage {
                            size: "5Gi".to_string(),
                            storage_class: None,
                        },
                        resources: ResourceRequirements {
                            requests: None,
                            limits: None,
                        },
                    },
                },
                ferriskey: None,
                ingress: None,
            },
            status: None,
        }
    }

    fn edge() -> Edge {
        Edge::new("aether-dataplane", "aether-system")
    }

    fn route(backends: &[Backend]) -> Value {
        build_route(
            &instance("auth.acme.com"),
            "deployment-1",
            "tenant-a",
            &BTreeMap::new(),
            None,
            &edge(),
            backends,
        )
    }

    /// The whole point of the move: the route names a Gateway in another
    /// namespace. Dropping the namespace makes it resolve inside the tenant's
    /// own, where there is no Gateway, and the route is quietly never served.
    #[test]
    fn the_route_names_the_gateway_and_the_namespace_it_lives_in() {
        let route = route(&[Backend::new("/", "webapp", 80)]);
        let parent = &route["spec"]["parentRefs"][0];

        assert_eq!(parent["name"], "aether-dataplane");
        assert_eq!(parent["namespace"], "aether-system");
        assert_eq!(parent["kind"], "Gateway");
    }

    #[test]
    fn the_route_serves_the_hostname_the_instance_declares() {
        let route = route(&[Backend::new("/", "webapp", 80)]);

        assert_eq!(route["spec"]["hostnames"][0], "auth.acme.com");
    }

    /// FerrisKey is two services behind one hostname. Both have to be in the
    /// route, each with its own port: the API does not answer on 80.
    #[test]
    fn every_backend_keeps_its_own_prefix_and_port() {
        let route = route(&[
            Backend::new("/api", "deployment-1-api", 3333),
            Backend::new("/", "deployment-1-webapp", 80),
        ]);
        let rules = route["spec"]["rules"].as_array().expect("rules");

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0]["matches"][0]["path"]["value"], "/api");
        assert_eq!(rules[0]["backendRefs"][0]["name"], "deployment-1-api");
        assert_eq!(rules[0]["backendRefs"][0]["port"], 3333);
        assert_eq!(rules[1]["matches"][0]["path"]["value"], "/");
        assert_eq!(rules[1]["backendRefs"][0]["port"], 80);
    }

    #[test]
    fn a_route_is_ready_once_it_is_accepted_and_its_backends_resolve() {
        let status = json!({
            "parents": [{
                "conditions": [
                    { "type": "Accepted", "status": "True" },
                    { "type": "ResolvedRefs", "status": "True" },
                ]
            }]
        });

        assert!(route_is_ready(Some(&status)));
    }

    /// The case the old check could not see. An ingress pointing at a service
    /// that was never created exists perfectly well, so "the object is there"
    /// reported a healthy instance serving nothing.
    #[test]
    fn a_route_whose_backends_do_not_resolve_is_not_ready() {
        let status = json!({
            "parents": [{
                "conditions": [
                    { "type": "Accepted", "status": "True" },
                    { "type": "ResolvedRefs", "status": "False" },
                ]
            }]
        });

        assert!(!route_is_ready(Some(&status)));
    }

    #[test]
    fn a_route_the_gateway_refused_is_not_ready() {
        let status = json!({
            "parents": [{
                "conditions": [
                    { "type": "Accepted", "status": "False" },
                    { "type": "ResolvedRefs", "status": "True" },
                ]
            }]
        });

        assert!(!route_is_ready(Some(&status)));
    }

    /// A route the controller has not looked at yet has no parents at all.
    /// Reading that as ready would report an instance as serving in the
    /// window before anything was programmed.
    #[test]
    fn a_route_with_no_status_yet_is_not_ready() {
        assert!(!route_is_ready(None));
        assert!(!route_is_ready(Some(&json!({}))));
        assert!(!route_is_ready(Some(&json!({ "parents": [] }))));
    }

    /// Two Gateways, one of which took the route. Accepted by either is
    /// enough; requiring all of them would make adding a second parent break
    /// an instance that was serving fine.
    #[test]
    fn one_parent_serving_it_is_enough() {
        let status = json!({
            "parents": [
                { "conditions": [{ "type": "Accepted", "status": "False" }] },
                { "conditions": [
                    { "type": "Accepted", "status": "True" },
                    { "type": "ResolvedRefs", "status": "True" },
                ]}
            ]
        });

        assert!(route_is_ready(Some(&status)));
    }

    #[test]
    fn an_instance_says_nothing_and_is_exposed() {
        assert!(exposed(&instance("auth.acme.com")));
    }

    #[test]
    fn an_instance_can_ask_not_to_be_exposed() {
        let mut instance = instance("auth.acme.com");
        instance.spec.ingress = Some(IngressConfig {
            enabled: false,
            class_name: None,
            tls: None,
        });

        assert!(!exposed(&instance));
    }
}
