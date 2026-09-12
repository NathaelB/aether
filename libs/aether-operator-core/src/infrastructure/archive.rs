//! Turning an instance's backup settings into what CloudNativePG understands.
//!
//! Everything here is a pure function from a spec to a resource. That is the
//! same shape as `edge.rs` next door, and for the same reason: what these build
//! is worth testing, and none of it is worth a cluster to test.
//!
//! The platform hands CloudNativePG the destination, the credentials and the
//! schedule. It deliberately does **not** hand it a retention policy. Barman's
//! retention is a time window and nothing else, so setting it to the
//! platform's window would delete the last archives of a deployment that
//! stopped being backed up -- which is precisely the case the platform's own
//! retention rule exists to protect. Deleting archives stays with whoever can
//! read both rules.

use std::collections::BTreeMap;

use aether_crds::v1alpha::{
    identity_instance::{BackupConfig, IdentityInstance},
    identity_instance_backup::ArchiveMethod,
};
use chrono::{DateTime, NaiveTime, Timelike, Utc};
use chrono_tz::Tz;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::core::{ApiResource, GroupVersionKind};
use serde_json::{Value, json};

const CNPG_GROUP: &str = "postgresql.cnpg.io";
const CNPG_VERSION: &str = "v1";

pub fn cnpg_backup_api_resource() -> ApiResource {
    ApiResource::from_gvk(&GroupVersionKind::gvk(CNPG_GROUP, CNPG_VERSION, "Backup"))
}

pub fn cnpg_scheduled_backup_api_resource() -> ApiResource {
    ApiResource::from_gvk(&GroupVersionKind::gvk(
        CNPG_GROUP,
        CNPG_VERSION,
        "ScheduledBackup",
    ))
}

/// The `backup` section of the CloudNativePG `Cluster`.
///
/// `None` when the instance archives nowhere, and the caller leaves the field
/// off entirely rather than writing an empty object: an empty
/// `barmanObjectStore` is a destination of `""`, which CloudNativePG accepts
/// and then fails on at archive time.
pub fn cluster_backup_section(config: Option<&BackupConfig>) -> Option<Value> {
    let config = config?;

    let mut store = json!({
        "destinationPath": config.destination_path,
        "s3Credentials": {
            "accessKeyId": {
                "name": config.credentials_secret,
                "key": "ACCESS_KEY_ID",
            },
            "secretAccessKey": {
                "name": config.credentials_secret,
                "key": "ACCESS_SECRET_KEY",
            },
        },
        // Compressed before it is encrypted, which is the only order that
        // works: encrypted bytes do not compress.
        "data": { "compression": "gzip" },
        "wal": { "compression": "gzip" },
    });

    if let Some(endpoint) = &config.endpoint_url {
        store["endpointURL"] = json!(endpoint);
    }

    if let Some(encryption) = &config.encryption {
        store["data"]["encryption"] = json!(encryption);
        store["wal"]["encryption"] = json!(encryption);
    }

    // No retentionPolicy. See the module docs: barman would enforce a window
    // the platform's own rule is a union with, and the two disagree in the one
    // direction that loses data.
    Some(json!({ "barmanObjectStore": store }))
}

/// A one shot archive of a cluster.
pub fn build_cnpg_backup(
    name: &str,
    namespace: &str,
    cluster: &str,
    method: ArchiveMethod,
    labels: &BTreeMap<String, String>,
    owner_reference: Option<OwnerReference>,
) -> Value {
    json!({
        "apiVersion": format!("{CNPG_GROUP}/{CNPG_VERSION}"),
        "kind": "Backup",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": labels,
            "ownerReferences": owner_reference.map(|owner| vec![owner]),
        },
        "spec": {
            "cluster": { "name": cluster },
            "method": method.to_string(),
        }
    })
}

/// What a recurring archive needs to know about itself.
///
/// A struct rather than eight positional arguments. Four of them are strings,
/// and a call site that swapped the namespace for the cluster name would
/// compile and archive somewhere nobody meant.
pub struct ScheduledArchive<'a> {
    pub name: &'a str,
    pub namespace: &'a str,
    pub cluster: &'a str,
    pub schedule: &'a str,
    pub method: ArchiveMethod,
    pub enabled: bool,
}

/// A recurring archive of a cluster.
///
/// A schedule that is off is written as `suspend: true` rather than deleted.
/// Deleting it would lose the cron somebody chose, and turning backups back on
/// would mean writing it out again from memory.
pub fn build_cnpg_scheduled_backup(
    archive: &ScheduledArchive<'_>,
    labels: &BTreeMap<String, String>,
    owner_reference: Option<OwnerReference>,
) -> Value {
    let ScheduledArchive {
        name,
        namespace,
        cluster,
        schedule,
        method,
        enabled,
    } = *archive;

    json!({
        "apiVersion": format!("{CNPG_GROUP}/{CNPG_VERSION}"),
        "kind": "ScheduledBackup",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": labels,
            "ownerReferences": owner_reference.map(|owner| vec![owner]),
        },
        "spec": {
            "cluster": { "name": cluster },
            "schedule": schedule,
            "method": method.to_string(),
            "suspend": !enabled,
            // The first archive waits for the schedule. Taking one the moment
            // a schedule is written would mean every configuration change
            // starts a backup, and a customer editing a cron at midday would
            // get a full base backup at midday.
            "immediate": false,
        }
    })
}

/// What is written beside an archive so it can be understood without the
/// control plane's database.
///
/// The archive itself says what is in it. This says what it belonged to: which
/// product, which version, which hostname, how it was sized, who could reach
/// it. Losing the control plane and keeping the bucket should leave somebody
/// able to rebuild the instance, and that is only true if the spec is in the
/// bucket too.
pub fn build_manifest(instance: &IdentityInstance, backup_name: &str, taken_at: &str) -> Value {
    json!({
        "aetherManifestVersion": 1,
        "backup": backup_name,
        "takenAt": taken_at,
        "instance": {
            "name": instance.metadata.name,
            "namespace": instance.metadata.namespace,
            "organisationId": instance.spec.organisation_id,
            "provider": instance.spec.provider,
            "version": instance.spec.version,
            "hostname": instance.spec.hostname,
            "database": instance.spec.database,
            "allowedCidrs": instance.spec.allowed_cidrs,
        }
    })
}

/// Splits `s3://bucket/some/prefix` into the bucket and the prefix.
///
/// Refuses anything else rather than guessing. A destination that is not an
/// S3 URL is a misconfiguration, and the only thing worse than failing on it
/// is writing an archive somewhere nobody is looking for it.
pub fn split_destination(destination: &str) -> Option<(&str, &str)> {
    let rest = destination.strip_prefix("s3://")?;
    let (bucket, prefix) = rest.split_once('/')?;

    if bucket.is_empty() || prefix.is_empty() {
        return None;
    }

    Some((bucket, prefix.trim_end_matches('/')))
}

#[cfg(test)]
mod tests {
    use aether_crds::common::types::ResourceRequirements;
    use aether_crds::v1alpha::identity_instance::{
        DatabaseConfig, DatabaseMode, IdentityInstanceSpec, IdentityProvider, ManagedClusterConfig,
        ManagedClusterStorage,
    };
    use kube::core::ObjectMeta;

    use super::*;

    fn config() -> BackupConfig {
        BackupConfig {
            destination_path: "s3://aether-backups/org-1/deployment-1".to_string(),
            endpoint_url: Some("http://rustfs:9000".to_string()),
            credentials_secret: "aether-object-store".to_string(),
            encryption: Some("AES256".to_string()),
        }
    }

    fn instance(backup: Option<BackupConfig>) -> IdentityInstance {
        IdentityInstance {
            metadata: ObjectMeta {
                name: Some("deployment-1".to_string()),
                namespace: Some("tenant-a".to_string()),
                ..Default::default()
            },
            spec: IdentityInstanceSpec {
                organisation_id: "org-1".to_string(),
                provider: IdentityProvider::Keycloak,
                version: "26.0.0".to_string(),
                hostname: "auth.acme.com".to_string(),
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
                allowed_cidrs: Some(vec!["203.0.113.0/24".to_string()]),
                backup,
            },
            status: None,
        }
    }

    /// An instance that archives nowhere gets no `backup` section at all.
    /// An empty one is a destination of "", which CloudNativePG accepts and
    /// then fails on at archive time, hours later and somewhere else.
    #[test]
    fn an_instance_that_archives_nowhere_has_no_backup_section() {
        assert!(cluster_backup_section(None).is_none());
    }

    #[test]
    fn the_cluster_writes_where_the_control_plane_said() {
        let section = cluster_backup_section(Some(&config())).expect("a section");
        let store = &section["barmanObjectStore"];

        assert_eq!(
            store["destinationPath"],
            "s3://aether-backups/org-1/deployment-1"
        );
        assert_eq!(store["endpointURL"], "http://rustfs:9000");
        assert_eq!(
            store["s3Credentials"]["accessKeyId"]["name"],
            "aether-object-store"
        );
    }

    /// The rule the module exists to state. Barman's retention is a window and
    /// nothing else, so handing it the platform's window would delete the last
    /// archives of a deployment that stopped being backed up.
    #[test]
    fn the_cluster_is_never_given_a_retention_policy() {
        let section = cluster_backup_section(Some(&config())).expect("a section");

        assert!(
            section.get("retentionPolicy").is_none(),
            "barman was handed a retention window it would enforce on its own"
        );
    }

    #[test]
    fn both_the_data_and_the_wals_are_encrypted() {
        let section = cluster_backup_section(Some(&config())).expect("a section");
        let store = &section["barmanObjectStore"];

        // WALs hold the same rows the data does, a few minutes later. An
        // installation encrypting one and not the other has encrypted nothing.
        assert_eq!(store["data"]["encryption"], "AES256");
        assert_eq!(store["wal"]["encryption"], "AES256");
    }

    #[test]
    fn a_bucket_policy_is_left_alone_when_nothing_was_asked_for() {
        let mut config = config();
        config.encryption = None;

        let section = cluster_backup_section(Some(&config)).expect("a section");

        assert!(section["barmanObjectStore"]["data"]["encryption"].is_null());
    }

    #[test]
    fn a_schedule_that_is_off_is_suspended_rather_than_dropped() {
        let suspended = build_cnpg_scheduled_backup(
            &ScheduledArchive {
                name: "deployment-1",
                namespace: "tenant-a",
                cluster: "deployment-1-db",
                schedule: "0 30 2 * * *",
                method: ArchiveMethod::BarmanObjectStore,
                enabled: false,
            },
            &BTreeMap::new(),
            None,
        );

        assert_eq!(suspended["spec"]["suspend"], true);
        // The cron survives being turned off, so turning it back on does not
        // mean writing it out again from memory.
        assert_eq!(suspended["spec"]["schedule"], "0 30 2 * * *");
    }

    #[test]
    fn a_schedule_does_not_archive_the_moment_it_is_written() {
        let schedule = build_cnpg_scheduled_backup(
            &ScheduledArchive {
                name: "deployment-1",
                namespace: "tenant-a",
                cluster: "deployment-1-db",
                schedule: "0 30 2 * * *",
                method: ArchiveMethod::BarmanObjectStore,
                enabled: true,
            },
            &BTreeMap::new(),
            None,
        );

        // Otherwise every edit to a cron starts a full base backup, and a
        // customer changing their window at midday gets one at midday.
        assert_eq!(schedule["spec"]["immediate"], false);
    }

    #[test]
    fn a_backup_names_the_cluster_and_the_method() {
        let backup = build_cnpg_backup(
            "deployment-1-20260912",
            "tenant-a",
            "deployment-1-db",
            ArchiveMethod::VolumeSnapshot,
            &BTreeMap::new(),
            None,
        );

        assert_eq!(backup["spec"]["cluster"]["name"], "deployment-1-db");
        assert_eq!(backup["spec"]["method"], "volumeSnapshot");
    }

    /// The point of writing it at all: losing the control plane's database and
    /// keeping the bucket should leave somebody able to rebuild the instance.
    #[test]
    fn the_manifest_holds_what_the_archive_alone_does_not_say() {
        let manifest = build_manifest(
            &instance(Some(config())),
            "deployment-1-20260912",
            "2026-09-12T02:30:00Z",
        );
        let described = &manifest["instance"];

        assert_eq!(described["version"], "26.0.0");
        assert_eq!(described["hostname"], "auth.acme.com");
        assert_eq!(described["organisationId"], "org-1");
        assert_eq!(described["allowedCidrs"][0], "203.0.113.0/24");
        assert_eq!(
            described["database"]["managedCluster"]["storage"]["size"],
            "5Gi"
        );
    }

    #[test]
    fn a_destination_splits_into_a_bucket_and_a_prefix() {
        assert_eq!(
            split_destination("s3://aether-backups/org-1/deployment-1"),
            Some(("aether-backups", "org-1/deployment-1"))
        );
        assert_eq!(
            split_destination("s3://aether-backups/org-1/deployment-1/"),
            Some(("aether-backups", "org-1/deployment-1"))
        );
    }

    /// Refused rather than guessed at. Writing an archive somewhere nobody is
    /// looking for it is worse than not writing one.
    #[test]
    fn a_destination_that_is_not_an_s3_url_is_refused() {
        for refused in [
            "aether-backups/org-1",
            "s3://aether-backups",
            "s3:///org-1",
            "https://example.com/bucket",
            "",
        ] {
            assert!(
                split_destination(refused).is_none(),
                "'{refused}' was accepted as a destination"
            );
        }
    }
}

/// The credentials the data plane archives with.
///
/// Read from the environment rather than discovered, the same way [`crate::infrastructure::edge::Edge`]
/// is. An operator that picked up whichever credentials it could find would
/// write a tenant's archive somewhere nobody chose, and the failure would look
/// like a missing backup rather than an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveStore {
    pub endpoint_url: Option<String>,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

/// The keys CloudNativePG's `s3Credentials` points at. Fixed rather than
/// configurable: both sides of this are written here, and a name that can
/// differ between them is a name that eventually does.
pub const ACCESS_KEY_ID: &str = "ACCESS_KEY_ID";
pub const ACCESS_SECRET_KEY: &str = "ACCESS_SECRET_KEY";

impl ArchiveStore {
    /// `None` when this data plane archives nothing, which is what an absent
    /// access key means. Distinct from a store being unreachable, and logged
    /// as the decision it is.
    pub fn from_env() -> Option<Self> {
        let access_key_id = non_empty("OBJECT_STORE_ACCESS_KEY")?;
        let secret_access_key = non_empty("OBJECT_STORE_SECRET_KEY")?;

        Some(Self {
            endpoint_url: non_empty("OBJECT_STORE_ENDPOINT"),
            region: non_empty("OBJECT_STORE_REGION").unwrap_or_else(|| "us-east-1".to_string()),
            access_key_id,
            secret_access_key,
        })
    }

    /// The secret CloudNativePG reads the credentials from, in the tenant's own
    /// namespace.
    ///
    /// CloudNativePG resolves secret references inside the cluster's namespace,
    /// so there is no arrangement where one secret in `aether-system` serves
    /// every tenant. What there is instead is one secret per namespace, written
    /// by the operator from configuration it already holds.
    pub fn credentials_secret(&self, name: &str, namespace: &str) -> Value {
        json!({
            "apiVersion": "v1",
            "kind": "Secret",
            "metadata": {
                "name": name,
                "namespace": namespace,
                "labels": {
                    "app.kubernetes.io/managed-by": "aether",
                },
            },
            "type": "Opaque",
            "stringData": {
                ACCESS_KEY_ID: self.access_key_id,
                ACCESS_SECRET_KEY: self.secret_access_key,
            }
        })
    }
}

fn non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod store_tests {
    use super::*;

    fn store() -> ArchiveStore {
        ArchiveStore {
            endpoint_url: Some("http://rustfs:9000".to_string()),
            region: "us-east-1".to_string(),
            access_key_id: "aether".to_string(),
            secret_access_key: "aetheraether".to_string(),
        }
    }

    /// CloudNativePG resolves secret references inside the cluster's own
    /// namespace, so the keys on both sides of that reference are written in
    /// one place and are not configurable.
    #[test]
    fn the_secret_carries_the_keys_the_cluster_reads() {
        let secret = store().credentials_secret("aether-object-store", "tenant-a");

        assert_eq!(secret["metadata"]["namespace"], "tenant-a");
        assert_eq!(secret["stringData"][ACCESS_KEY_ID], "aether");
        assert_eq!(secret["stringData"][ACCESS_SECRET_KEY], "aetheraether");
    }

    #[test]
    fn the_cluster_and_the_secret_agree_on_the_key_names() {
        let section = cluster_backup_section(Some(&BackupConfig {
            destination_path: "s3://aether-backups/org-1/deployment-1".to_string(),
            endpoint_url: None,
            credentials_secret: "aether-object-store".to_string(),
            encryption: None,
        }))
        .expect("a section");
        let credentials = &section["barmanObjectStore"]["s3Credentials"];
        let secret = store().credentials_secret("aether-object-store", "tenant-a");

        // The one pair of names that has to match across two resources the
        // operator writes at different moments.
        assert_eq!(credentials["accessKeyId"]["key"], ACCESS_KEY_ID);
        assert_eq!(credentials["secretAccessKey"]["key"], ACCESS_SECRET_KEY);
        assert!(secret["stringData"][ACCESS_KEY_ID].is_string());
        assert!(secret["stringData"][ACCESS_SECRET_KEY].is_string());
    }
}

/// Rewrites a local cron expression into the UTC one CloudNativePG will accept.
///
/// CloudNativePG cannot express a zone. Its admission webhook counts whitespace
/// separated fields and refuses anything but five or six, so a `CRON_TZ=`
/// prefix is rejected at apply time -- found by applying one, not by reading
/// the documentation, which points at a parser that does support it.
///
/// So the conversion happens here, and it happens on **every reconcile**. That
/// is the part that matters: an offset computed once and stored would be an
/// hour wrong from every daylight saving change until somebody noticed, and
/// wrong in the direction of running a backup during the working day.
/// Recomputed on a loop that already runs every thirty seconds, the same change
/// corrects itself before anyone is awake to see it.
///
/// Returns the expression unchanged when it cannot be understood. A schedule
/// the platform cannot parse is still a schedule somebody wrote, and handing it
/// on unchanged lets CloudNativePG give its own opinion rather than having this
/// function silently invent a different time.
pub fn to_utc_cron(local: &str, zone: Tz, on: DateTime<Utc>) -> String {
    let fields: Vec<&str> = local.split_whitespace().collect();
    if fields.len() != 6 {
        return local.to_string();
    }

    let (Ok(minute), Ok(hour)) = (fields[1].parse::<u32>(), fields[2].parse::<u32>()) else {
        return local.to_string();
    };

    let Some(time) = NaiveTime::from_hms_opt(hour, minute, 0) else {
        return local.to_string();
    };

    // Anchored on the day being reconciled, which is what makes this follow
    // daylight saving rather than freeze one side of it.
    let today = on.with_timezone(&zone).date_naive();
    let Some(local_instant) = today.and_time(time).and_local_timezone(zone).single() else {
        // Either the hour does not exist on this date, which happens once a
        // year when the clocks go forward, or it exists twice. Neither is worth
        // guessing at: the expression goes on unchanged and the backup runs at
        // whatever UTC reads it as, one time.
        return local.to_string();
    };

    let utc = local_instant.with_timezone(&Utc);
    let day_shift = (utc.date_naive() - today).num_days();

    let day_of_week = match fields[5] {
        // A daily schedule still runs once a day whichever side of midnight
        // the conversion lands on, so the shift is irrelevant to it.
        "*" => "*".to_string(),
        other => match other.parse::<i64>() {
            // Sunday 00:30 in Paris is Saturday 23:30 in UTC. A weekly
            // schedule that did not move its day would run on the wrong one.
            Ok(day) => (((day + day_shift) % 7 + 7) % 7).to_string(),
            Err(_) => return local.to_string(),
        },
    };

    format!(
        "{} {} {} {} {} {}",
        fields[0],
        utc.minute(),
        utc.hour(),
        fields[3],
        fields[4],
        day_of_week
    )
}

#[cfg(test)]
mod cron_tests {
    use super::*;

    fn at(raw: &str) -> DateTime<Utc> {
        raw.parse().expect("an instant")
    }

    /// The whole reason this function exists. Applying a `CRON_TZ=` prefixed
    /// expression is refused by CloudNativePG's webhook with "Expected 5 to 6
    /// fields, found 7".
    #[test]
    fn the_result_is_always_six_fields() {
        let converted = to_utc_cron(
            "0 30 2 * * *",
            chrono_tz::Europe::Paris,
            at("2026-01-15T00:00:00Z"),
        );

        assert_eq!(converted.split_whitespace().count(), 6);
        assert!(!converted.contains('='));
    }

    /// Paris is one hour ahead of UTC in winter.
    #[test]
    fn a_winter_morning_in_paris_is_an_hour_earlier_in_utc() {
        assert_eq!(
            to_utc_cron(
                "0 30 2 * * *",
                chrono_tz::Europe::Paris,
                at("2026-01-15T00:00:00Z")
            ),
            "0 30 1 * * *"
        );
    }

    /// And two hours ahead in summer. The same schedule, the same zone, a
    /// different expression -- which is why this is recomputed on every
    /// reconcile rather than stored.
    #[test]
    fn the_same_schedule_moves_when_the_clocks_do() {
        assert_eq!(
            to_utc_cron(
                "0 30 2 * * *",
                chrono_tz::Europe::Paris,
                at("2026-07-15T00:00:00Z")
            ),
            "0 30 0 * * *"
        );
    }

    /// Sunday 00:30 in Paris is Saturday 23:30 in UTC. A weekly schedule that
    /// kept its day would run a whole day out.
    #[test]
    fn a_weekly_schedule_that_crosses_midnight_moves_its_day() {
        assert_eq!(
            to_utc_cron(
                "0 30 0 * * 0",
                chrono_tz::Europe::Paris,
                at("2026-01-15T00:00:00Z")
            ),
            "0 30 23 * * 6"
        );
    }

    /// A daily schedule runs once a day whichever side of midnight it lands
    /// on, so there is no day to move.
    #[test]
    fn a_daily_schedule_keeps_its_wildcard_day() {
        assert_eq!(
            to_utc_cron(
                "0 30 0 * * *",
                chrono_tz::Europe::Paris,
                at("2026-01-15T00:00:00Z")
            ),
            "0 30 23 * * *"
        );
    }

    #[test]
    fn utc_converts_to_itself() {
        assert_eq!(
            to_utc_cron("0 30 2 * * 0", Tz::UTC, at("2026-01-15T00:00:00Z")),
            "0 30 2 * * 0"
        );
    }

    /// Handed on unchanged rather than replaced with a guess. A schedule the
    /// platform cannot read is still one somebody wrote, and CloudNativePG
    /// gets to give its own opinion on it.
    #[test]
    fn an_expression_this_cannot_read_is_passed_through() {
        for opaque in ["*/5 * * * * *", "0 30 2 * *", "nonsense", ""] {
            assert_eq!(
                to_utc_cron(opaque, chrono_tz::Europe::Paris, at("2026-01-15T00:00:00Z")),
                opaque
            );
        }
    }
}
