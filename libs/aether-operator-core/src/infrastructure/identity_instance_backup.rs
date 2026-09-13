//! The controllers behind the two backup kinds.
//!
//! Both do the same small thing: turn an Aether resource into the
//! CloudNativePG one that carries it out, and report back what happened. The
//! archive itself is CloudNativePG's work from start to finish. What this adds
//! is the part CloudNativePG has no opinion about -- which instance an archive
//! belonged to, and what that instance looked like.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use aether_crds::v1alpha::{
    identity_instance::IdentityInstance,
    identity_instance_backup::{
        IdentityInstanceBackup, IdentityInstanceBackupSchedule,
        IdentityInstanceBackupScheduleStatus, IdentityInstanceBackupSpec,
        IdentityInstanceBackupStatus,
    },
    identity_instance_upgrade::IdentityInstanceRef as CrdIdentityInstanceRef,
};
use chrono_tz::Tz;
use futures::StreamExt;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{OwnerReference, Time};
use kube::{
    Api, Client, Resource, ResourceExt,
    api::{DynamicObject, Patch, PatchParams},
    runtime::{Controller, controller::Action, watcher},
};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{
    domain::OperatorError,
    infrastructure::archive::{
        ScheduledArchive, build_cnpg_backup, build_cnpg_scheduled_backup, build_manifest,
        cnpg_backup_api_resource, cnpg_cluster_api_resource, cnpg_scheduled_backup_api_resource,
        split_destination, to_utc_cron,
    },
};

/// How long before a resource is looked at again when nothing happened.
///
/// An archive takes minutes, so polling faster only produces log lines. Long
/// enough to be quiet, short enough that a finished backup is recorded while
/// somebody is still watching it.
const REQUEUE: Duration = Duration::from_secs(30);

/// And how long before a failure is retried.
const REQUEUE_AFTER_ERROR: Duration = Duration::from_secs(60);

struct BackupContext {
    client: Client,
    manifests: Arc<dyn ArchiveObjects>,
}

/// Writing the manifest beside an archive.
///
/// A trait so the controller can be reasoned about without a bucket.
#[async_trait::async_trait]
pub trait ArchiveObjects: Send + Sync {
    async fn write(
        &self,
        destination: &str,
        object_key: &str,
        body: Vec<u8>,
    ) -> Result<(), OperatorError>;

    /// What the archive under this prefix weighs, in bytes.
    ///
    /// Measured rather than read off the `Backup`: CloudNativePG reports a
    /// begin and end WAL, a phase and a destination, and no size at all for a
    /// barman archive. The control plane refuses a report without one -- an
    /// archive of no bytes is a backup that did not happen -- so without this
    /// nothing a data plane takes is ever recorded.
    ///
    /// `None` when the store could not be asked. Distinct from zero on
    /// purpose: one is a measurement nobody took, and the other is an archive
    /// that does not exist.
    async fn measure(&self, destination: &str, prefix: &str) -> Option<u64>;
}

pub async fn run(manifests: Arc<dyn ArchiveObjects>) -> Result<(), OperatorError> {
    info!("Starting IdentityInstanceBackup controllers");

    let client = Client::try_default()
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?;

    let schedules = Api::<IdentityInstanceBackupSchedule>::all(client.clone());
    let backups = Api::<IdentityInstanceBackup>::all(client.clone());

    let schedule_context = Arc::new(BackupContext {
        client: client.clone(),
        manifests: manifests.clone(),
    });
    let adoption_context = Arc::new(BackupContext {
        client: client.clone(),
        manifests: manifests.clone(),
    });
    let client_for_adoption = client.clone();
    let backup_context = Arc::new(BackupContext { client, manifests });

    let schedules = Controller::new(schedules, watcher::Config::default())
        .run(reconcile_schedule, schedule_error_policy, schedule_context)
        .for_each(|_| async {});

    let backups = Controller::new(backups, watcher::Config::default())
        .run(reconcile_backup, backup_error_policy, backup_context)
        .for_each(|_| async {});

    // A `ScheduledBackup` creates CloudNativePG `Backup` objects directly, and
    // nothing at this layer knew they existed: the Aether kind was only ever
    // created for a one-off archive, and nothing created those either. So
    // every archive that actually happened was invisible to the platform that
    // scheduled it. This adopts them.
    let adopted = Controller::new_with(
        Api::<DynamicObject>::all_with(client_for_adoption.clone(), &cnpg_backup_api_resource()),
        watcher::Config::default(),
        cnpg_backup_api_resource(),
    )
    .run(adopt_cnpg_backup, adoption_error_policy, adoption_context)
    .for_each(|_| async {});

    futures::join!(schedules, backups, adopted);

    Ok(())
}

/// Gives a CloudNativePG `Backup` an Aether resource to be seen through.
///
/// Only for clusters this platform runs: a store shared with somebody else's
/// CloudNativePG is a supported situation, and adopting their archives would
/// report backups of databases Aether does not run.
async fn adopt_cnpg_backup(
    cnpg: Arc<DynamicObject>,
    context: Arc<BackupContext>,
) -> Result<Action, OperatorError> {
    let name = cnpg.name_any();
    let namespace = cnpg.namespace().ok_or(OperatorError::MissingName)?;

    let Some(cluster) = cnpg
        .data
        .get("spec")
        .and_then(|spec| spec.get("cluster"))
        .and_then(|cluster| cluster.get("name"))
        .and_then(Value::as_str)
    else {
        return Ok(Action::await_change());
    };

    let Some(instance_name) = instance_of_cluster(cluster) else {
        return Ok(Action::await_change());
    };

    let instances: Api<IdentityInstance> = Api::namespaced(context.client.clone(), &namespace);
    if instances
        .get_opt(instance_name)
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?
        .is_none()
    {
        return Ok(Action::await_change());
    }

    let backups: Api<IdentityInstanceBackup> = Api::namespaced(context.client.clone(), &namespace);
    if backups
        .get_opt(&name)
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?
        .is_some()
    {
        // Either already adopted, or one this platform asked for itself. The
        // other controller follows it from here.
        return Ok(Action::await_change());
    }

    info!(
        backup = %name,
        instance = %instance_name,
        "adopting an archive taken on a schedule"
    );

    backups
        .patch(
            &name,
            &PatchParams::apply("aether-operator").force(),
            &Patch::Apply(&adopted_backup(&name, &namespace, instance_name)),
        )
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?;

    // The status is the only place the adoption is recorded, and it cannot be
    // set by the apply above: a create writes no status.
    patch_status(
        &backups,
        &name,
        json!({ "status": { "adoptedFrom": name } }),
    )
    .await?;

    Ok(Action::requeue(REQUEUE))
}

/// The Aether resource standing for an archive this platform did not start.
fn adopted_backup(name: &str, namespace: &str, instance: &str) -> IdentityInstanceBackup {
    IdentityInstanceBackup {
        metadata: kube::core::ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            labels: Some(labels(instance)),
            ..Default::default()
        },
        spec: IdentityInstanceBackupSpec {
            identity_instance_ref: CrdIdentityInstanceRef {
                name: instance.to_string(),
            },
            method: Default::default(),
        },
        status: None,
    }
}

fn adoption_error_policy(
    cnpg: Arc<DynamicObject>,
    error: &OperatorError,
    _context: Arc<BackupContext>,
) -> Action {
    error!(backup = %cnpg.name_any(), %error, "an archive could not be adopted");
    Action::requeue(REQUEUE_AFTER_ERROR)
}

fn labels(instance: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "app.kubernetes.io/managed-by".to_string(),
            "aether".to_string(),
        ),
        (
            "aether.dev/identity-instance".to_string(),
            instance.to_string(),
        ),
    ])
}

/// The CloudNativePG cluster behind an instance.
///
/// Rebuilt from the instance name rather than read off the cluster, because
/// this runs before there is any guarantee the cluster still exists: a backup
/// of an instance somebody is deleting has to fail saying so, not panic.
fn cluster_name(instance: &str) -> String {
    format!("{instance}-db")
}

/// The instance a CloudNativePG cluster belongs to, or nothing when the
/// cluster is not one of ours.
///
/// The inverse of [`cluster_name`], and the only thing that decides whether an
/// archive on this cluster is the platform's business. A store shared with
/// somebody else's CloudNativePG is a supported situation, and adopting their
/// backups would report archives of databases this platform does not run.
fn instance_of_cluster(cluster: &str) -> Option<&str> {
    cluster.strip_suffix("-db").filter(|name| !name.is_empty())
}

/// Where the manifest beside an archive is written, relative to the
/// deployment's prefix.
///
/// One definition, used by the writer and by the status that reports it.
/// Deriving it twice is how the two come to disagree.
fn manifest_key(backup_name: &str) -> String {
    format!("aether/{backup_name}.json")
}

/// The Postgres major a cluster is running.
///
/// Two places say it and neither is guaranteed: newer CloudNativePG reports it
/// as a number, older ones only carry the image it runs. Both are read because
/// an archive that cannot say which engine wrote it is one nobody can safely
/// restore -- and a physical archive restored onto the wrong major does not
/// fail politely.
fn postgres_major_of(cluster: &Value) -> Option<u32> {
    let reported = cluster
        .get("status")
        .and_then(|status| status.get("pgDataImageInfo"))
        .and_then(|info| info.get("majorVersion"))
        .and_then(Value::as_u64);

    if let Some(major) = reported {
        return u32::try_from(major).ok();
    }

    let image = cluster
        .get("status")
        .and_then(|status| status.get("image"))
        .or_else(|| cluster.get("spec").and_then(|spec| spec.get("imageName")))
        .and_then(Value::as_str)?;

    major_from_image(image)
}

/// `ghcr.io/cloudnative-pg/postgresql:17.2-standard-trixie` is 17.
///
/// The tag is taken after the last colon so a registry carrying a port does
/// not read as a version, and the major is what precedes the first dot.
fn major_from_image(image: &str) -> Option<u32> {
    let tag = image.rsplit_once(':')?.1;

    tag.split(['.', '-'])
        .next()
        .filter(|major| !major.is_empty())
        .and_then(|major| major.parse().ok())
}

async fn reconcile_schedule(
    schedule: Arc<IdentityInstanceBackupSchedule>,
    context: Arc<BackupContext>,
) -> Result<Action, OperatorError> {
    let name = schedule.name_any();
    let namespace = schedule.namespace().ok_or(OperatorError::MissingName)?;
    let instance = schedule.spec.identity_instance_ref.name.clone();

    let owner = owner_reference(schedule.as_ref());

    // Converted here, on every reconcile, because CloudNativePG cannot express
    // a zone: its webhook counts fields and refuses the `CRON_TZ=` prefix. An
    // offset computed once and stored would be an hour wrong from every
    // daylight saving change until somebody noticed; recomputed on a loop that
    // already runs, the same change corrects itself overnight.
    let zone: Tz = schedule.spec.zone.parse().unwrap_or_else(|_| {
        warn!(
            schedule = %name,
            zone = %schedule.spec.zone,
            "that is not a zone: the schedule will be read as UTC"
        );
        Tz::UTC
    });
    let utc_schedule = to_utc_cron(&schedule.spec.schedule, zone, chrono::Utc::now());

    let desired = build_cnpg_scheduled_backup(
        &ScheduledArchive {
            name: &name,
            namespace: &namespace,
            cluster: &cluster_name(&instance),
            schedule: &utc_schedule,
            method: schedule.spec.method,
            enabled: schedule.spec.enabled,
        },
        &labels(&instance),
        owner,
    );

    let scheduled: Api<DynamicObject> = Api::namespaced_with(
        context.client.clone(),
        &namespace,
        &cnpg_scheduled_backup_api_resource(),
    );

    scheduled
        .patch(
            &name,
            &PatchParams::apply("aether-operator").force(),
            &Patch::Apply(&desired),
        )
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?;

    let status = IdentityInstanceBackupScheduleStatus {
        cnpg_scheduled_backup_name: Some(name.clone()),
        last_scheduled_at: None,
        conditions: Vec::new(),
        error: None,
    };

    let schedules: Api<IdentityInstanceBackupSchedule> =
        Api::namespaced(context.client.clone(), &namespace);
    patch_status(&schedules, &name, json!({ "status": status })).await?;

    info!(
        schedule = %name,
        instance = %instance,
        local = %schedule.spec.schedule,
        zone = %zone,
        utc = %utc_schedule,
        enabled = schedule.spec.enabled,
        "the archive schedule is applied"
    );

    Ok(Action::requeue(REQUEUE))
}

async fn reconcile_backup(
    backup: Arc<IdentityInstanceBackup>,
    context: Arc<BackupContext>,
) -> Result<Action, OperatorError> {
    let name = backup.name_any();
    let namespace = backup.namespace().ok_or(OperatorError::MissingName)?;
    let instance_name = backup.spec.identity_instance_ref.name.clone();

    let owner = owner_reference(backup.as_ref());

    let cnpg: Api<DynamicObject> = Api::namespaced_with(
        context.client.clone(),
        &namespace,
        &cnpg_backup_api_resource(),
    );

    // An adopted archive already exists and belongs to the `ScheduledBackup`
    // that made it. Applying our own on top would force the ownership over to
    // this resource, and deleting the schedule would then stop cascading to
    // the archives it took.
    let adopted_from = backup
        .status
        .as_ref()
        .and_then(|status| status.adopted_from.clone());

    if adopted_from.is_none() {
        let desired = build_cnpg_backup(
            &name,
            &namespace,
            &cluster_name(&instance_name),
            backup.spec.method,
            &labels(&instance_name),
            owner,
        );

        cnpg.patch(
            &name,
            &PatchParams::apply("aether-operator").force(),
            &Patch::Apply(&desired),
        )
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?;
    }

    let observed = cnpg
        .get_opt(&name)
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?;

    let instances: Api<IdentityInstance> = Api::namespaced(context.client.clone(), &namespace);
    let instance =
        instances
            .get_opt(&instance_name)
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?;

    // Written once, on the first reconcile that can see both the instance and
    // a destination. Rewriting it on every pass would replace a record of what
    // the instance looked like when the archive was taken with what it looks
    // like now, which is the one thing the manifest exists not to do.
    if let Some(instance) = instance.as_ref() {
        write_manifest_once(context.as_ref(), instance, &name, backup.as_ref()).await;
    } else {
        warn!(
            backup = %name,
            instance = %instance_name,
            "the instance this archive belongs to is gone: no manifest will be written"
        );
    }

    // Read from the cluster rather than from the archive: CloudNativePG does
    // not put the engine on a `Backup`, and the cluster is the only place the
    // answer exists while it still exists at all.
    let clusters: Api<DynamicObject> = Api::namespaced_with(
        context.client.clone(),
        &namespace,
        &cnpg_cluster_api_resource(),
    );
    let postgres_major = clusters
        .get_opt(&cluster_name(&instance_name))
        .await
        .ok()
        .flatten()
        .and_then(|cluster| postgres_major_of(&Value::Object(cluster.data.as_object()?.clone())));

    // Measured from the store, because the `Backup` does not say. Only once
    // the archive is complete: a prefix still being written weighs whatever
    // has landed so far, and reporting that would record an archive smaller
    // than the one that exists.
    let size_bytes = match (
        observed.as_ref().and_then(archive_prefix_of),
        instance
            .as_ref()
            .and_then(|instance| instance.spec.backup.as_ref()),
    ) {
        (Some(prefix), Some(config)) => {
            context
                .manifests
                .measure(&config.destination_path, &prefix)
                .await
        }
        _ => None,
    };

    let status = status_from_cnpg(
        observed.as_ref(),
        &name,
        instance.as_ref(),
        postgres_major,
        adopted_from,
        size_bytes,
    );
    let backups: Api<IdentityInstanceBackup> = Api::namespaced(context.client.clone(), &namespace);
    patch_status(&backups, &name, json!({ "status": status })).await?;

    Ok(Action::requeue(REQUEUE))
}

/// Where barman put this archive, relative to the destination.
///
/// `<serverName>/base/<backupId>`, which is barman's layout and not a choice
/// this platform gets to make. `None` until the archive completes: the two
/// fields it is built from are written when it does.
fn archive_prefix_of(observed: &DynamicObject) -> Option<String> {
    let status = observed.data.get("status")?;

    if status.get("phase").and_then(Value::as_str) != Some("completed") {
        return None;
    }

    let server = status.get("serverName").and_then(Value::as_str)?;
    let backup = status.get("backupId").and_then(Value::as_str)?;

    Some(format!("{server}/base/{backup}"))
}

/// Mirrors what CloudNativePG says about the archive.
///
/// Everything is optional because everything is optional at some point in an
/// archive's life: a Backup that was created a second ago has no phase, no
/// size and no destination, and reporting zeroes for those would be inventing
/// facts about an archive that does not exist yet.
fn status_from_cnpg(
    observed: Option<&DynamicObject>,
    name: &str,
    instance: Option<&IdentityInstance>,
    postgres_major: Option<u32>,
    adopted_from: Option<String>,
    size_bytes: Option<u64>,
) -> IdentityInstanceBackupStatus {
    let status = observed.and_then(|object| object.data.get("status"));

    let string = |field: &str| -> Option<String> {
        status
            .and_then(|status| status.get(field))
            .and_then(Value::as_str)
            .map(ToString::to_string)
    };

    IdentityInstanceBackupStatus {
        phase: None,
        cnpg_backup_name: Some(name.to_string()),
        destination_path: string("destinationPath").or_else(|| {
            instance
                .and_then(|instance| instance.spec.backup.as_ref())
                .map(|backup| backup.destination_path.clone())
        }),
        // What the store says it holds, falling back to what CloudNativePG says
        // when a version of it reports one. Neither is invented: absent stays
        // absent, and the control plane refuses an archive that cannot say how
        // big it is rather than recording a zero.
        size_bytes: size_bytes
            .map(|bytes| bytes.to_string())
            .or_else(|| string("backupSize")),
        started_at: string("startedAt").map(|raw| Time(parse_time(&raw))),
        stopped_at: string("stoppedAt").map(|raw| Time(parse_time(&raw))),
        postgres_major: postgres_major.map(|major| major.to_string()),
        manifest_key: Some(manifest_key(name)),
        adopted_from,
        conditions: Vec::new(),
        error: string("error"),
    }
}

fn parse_time(raw: &str) -> chrono::DateTime<chrono::Utc> {
    raw.parse().unwrap_or_else(|_| chrono::Utc::now())
}

async fn write_manifest_once(
    context: &BackupContext,
    instance: &IdentityInstance,
    backup_name: &str,
    backup: &IdentityInstanceBackup,
) {
    // Already written: the status carries a destination, which only happens
    // after a pass that got this far.
    if backup
        .status
        .as_ref()
        .and_then(|status| status.destination_path.as_ref())
        .is_some()
    {
        return;
    }

    let Some(config) = instance.spec.backup.as_ref() else {
        return;
    };

    if split_destination(&config.destination_path).is_none() {
        error!(
            destination = %config.destination_path,
            "the destination is not an S3 URL: no manifest will be written"
        );
        return;
    }

    let taken_at = chrono::Utc::now().to_rfc3339();
    let manifest = build_manifest(instance, backup_name, &taken_at);
    let body = match serde_json::to_vec_pretty(&manifest) {
        Ok(body) => body,
        Err(error) => {
            error!(%error, "the manifest could not be written down");
            return;
        }
    };

    let key = format!("aether/{backup_name}.json");

    // A manifest that could not be written does not fail the archive. The
    // archive is the thing being protected; the manifest makes it easier to
    // understand later, and losing the second is not a reason to lose the
    // first.
    if let Err(error) = context
        .manifests
        .write(&config.destination_path, &key, body)
        .await
    {
        warn!(
            backup = %backup_name,
            %error,
            "the archive was taken and its manifest was not written: it will be harder to place if the control plane is lost"
        );
    }
}

async fn patch_status<K>(api: &Api<K>, name: &str, status: Value) -> Result<(), OperatorError>
where
    K: Clone + serde::de::DeserializeOwned + std::fmt::Debug,
{
    api.patch_status(name, &PatchParams::default(), &Patch::Merge(&status))
        .await
        .map(|_| ())
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })
}

fn owner_reference<K>(resource: &K) -> Option<OwnerReference>
where
    K: Resource<DynamicType = ()>,
{
    resource.controller_owner_ref(&())
}

fn schedule_error_policy(
    schedule: Arc<IdentityInstanceBackupSchedule>,
    error: &OperatorError,
    _context: Arc<BackupContext>,
) -> Action {
    error!(schedule = %schedule.name_any(), %error, "the archive schedule could not be applied");
    Action::requeue(REQUEUE_AFTER_ERROR)
}

fn backup_error_policy(
    backup: Arc<IdentityInstanceBackup>,
    error: &OperatorError,
    _context: Arc<BackupContext>,
) -> Action {
    error!(backup = %backup.name_any(), %error, "the archive could not be taken");
    Action::requeue(REQUEUE_AFTER_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The inverse of `cluster_name`, and what decides whether an archive is
    /// this platform's business at all.
    #[test]
    fn a_cluster_this_platform_runs_names_its_instance() {
        assert_eq!(
            instance_of_cluster(&cluster_name("deployment-abc")),
            Some("deployment-abc")
        );
    }

    #[test]
    fn a_cluster_somebody_else_runs_is_left_alone() {
        // A store shared with another CloudNativePG is supported. Adopting its
        // archives would report backups of databases Aether does not run.
        assert_eq!(instance_of_cluster("their-cluster"), None);
        assert_eq!(instance_of_cluster("-db"), None);
        assert_eq!(instance_of_cluster(""), None);
    }

    #[test]
    fn the_manifest_key_is_the_same_one_the_writer_uses() {
        // Derived in one place. Two derivations are two answers to "what is
        // this archive recorded under", and the control plane deduplicates on
        // exactly that.
        assert_eq!(
            manifest_key("nightly-20260913"),
            "aether/nightly-20260913.json"
        );
    }

    #[test]
    fn the_engine_is_read_from_what_the_cluster_reports() {
        let cluster = json!({
            "status": { "pgDataImageInfo": { "majorVersion": 17 } }
        });

        assert_eq!(postgres_major_of(&cluster), Some(17));
    }

    /// Older CloudNativePG reports no major, only the image it runs. An
    /// archive that cannot say which engine wrote it is one nobody can safely
    /// restore, so both are read.
    #[test]
    fn the_engine_falls_back_to_the_image_the_cluster_runs() {
        let cluster = json!({
            "status": { "image": "ghcr.io/cloudnative-pg/postgresql:17.2-standard-trixie" }
        });

        assert_eq!(postgres_major_of(&cluster), Some(17));
    }

    #[test]
    fn the_engine_is_read_from_the_spec_when_the_status_is_silent() {
        let cluster = json!({
            "spec": { "imageName": "ghcr.io/cloudnative-pg/postgresql:16.4" }
        });

        assert_eq!(postgres_major_of(&cluster), Some(16));
    }

    #[test]
    fn a_registry_carrying_a_port_does_not_read_as_a_version() {
        assert_eq!(
            major_from_image("registry.internal:5000/postgresql:15.6"),
            Some(15)
        );
    }

    #[test]
    fn an_engine_nobody_can_name_is_left_unset_rather_than_guessed() {
        assert_eq!(postgres_major_of(&json!({})), None);
        assert_eq!(major_from_image("postgresql"), None);
        assert_eq!(major_from_image("postgresql:latest"), None);
    }

    /// Barman's layout, not a choice this platform gets to make.
    #[test]
    fn a_completed_archive_says_where_barman_put_it() {
        let observed = DynamicObject {
            types: None,
            metadata: Default::default(),
            data: json!({
                "status": {
                    "phase": "completed",
                    "serverName": "deployment-abc-db",
                    "backupId": "20260913T013534",
                }
            }),
        };

        assert_eq!(
            archive_prefix_of(&observed).as_deref(),
            Some("deployment-abc-db/base/20260913T013534")
        );
    }

    /// A prefix still being written weighs whatever has landed so far.
    /// Measuring it would record an archive smaller than the one that exists.
    #[test]
    fn an_archive_still_running_is_not_measured() {
        for status in [
            json!({ "status": { "phase": "running", "serverName": "s", "backupId": "b" } }),
            json!({ "status": { "phase": "completed", "serverName": "s" } }),
            json!({ "status": { "phase": "completed", "backupId": "b" } }),
            json!({}),
        ] {
            let observed = DynamicObject {
                types: None,
                metadata: Default::default(),
                data: status,
            };

            assert!(archive_prefix_of(&observed).is_none());
        }
    }

    /// CloudNativePG reports a begin and end WAL, a phase and a destination,
    /// and no size at all for a barman archive. Nothing was ever recorded in
    /// the control plane because of it: a report without a size is refused.
    #[test]
    fn a_size_the_store_measured_is_what_gets_reported() {
        let observed = DynamicObject {
            types: None,
            metadata: Default::default(),
            data: json!({ "status": { "phase": "completed" } }),
        };

        assert_eq!(
            status_from_cnpg(Some(&observed), "n", None, None, None, Some(8192)).size_bytes,
            Some("8192".to_string())
        );
        assert_eq!(
            status_from_cnpg(Some(&observed), "n", None, None, None, None).size_bytes,
            None,
            "absent stays absent rather than becoming a zero nobody measured"
        );
    }

    #[test]
    fn an_adopted_archive_points_at_the_instance_it_belongs_to() {
        let adopted = adopted_backup("nightly-20260913", "tenant-a", "deployment-abc");

        assert_eq!(adopted.spec.identity_instance_ref.name, "deployment-abc");
        assert_eq!(adopted.metadata.namespace.as_deref(), Some("tenant-a"));
        assert_eq!(
            adopted.metadata.name.as_deref(),
            Some("nightly-20260913"),
            "named after the archive it follows, so a redelivery finds it"
        );
    }

    /// Present means "follow this, do not create it". Applying a `Backup` that
    /// already exists would force its ownership away from the `ScheduledBackup`
    /// that made it, and deleting the schedule would stop cascading.
    #[test]
    fn a_status_says_whether_the_archive_was_adopted() {
        let followed = status_from_cnpg(
            None,
            "nightly",
            None,
            Some(17),
            Some("nightly".into()),
            Some(4096),
        );
        assert_eq!(followed.adopted_from.as_deref(), Some("nightly"));
        assert_eq!(followed.postgres_major.as_deref(), Some("17"));
        assert_eq!(
            followed.manifest_key.as_deref(),
            Some("aether/nightly.json")
        );

        let asked_for = status_from_cnpg(None, "one-off", None, None, None, None);
        assert!(asked_for.adopted_from.is_none());
    }
}
