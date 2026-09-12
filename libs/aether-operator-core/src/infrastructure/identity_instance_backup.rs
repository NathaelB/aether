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
        IdentityInstanceBackupScheduleStatus, IdentityInstanceBackupStatus,
    },
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
        cnpg_backup_api_resource, cnpg_scheduled_backup_api_resource, split_destination,
        to_utc_cron,
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
    manifests: Arc<dyn ManifestWriter>,
}

/// Writing the manifest beside an archive.
///
/// A trait so the controller can be reasoned about without a bucket. It has
/// exactly one method because there is exactly one thing to write.
#[async_trait::async_trait]
pub trait ManifestWriter: Send + Sync {
    async fn write(
        &self,
        destination: &str,
        object_key: &str,
        body: Vec<u8>,
    ) -> Result<(), OperatorError>;
}

pub async fn run(manifests: Arc<dyn ManifestWriter>) -> Result<(), OperatorError> {
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
    let backup_context = Arc::new(BackupContext { client, manifests });

    let schedules = Controller::new(schedules, watcher::Config::default())
        .run(reconcile_schedule, schedule_error_policy, schedule_context)
        .for_each(|_| async {});

    let backups = Controller::new(backups, watcher::Config::default())
        .run(reconcile_backup, backup_error_policy, backup_context)
        .for_each(|_| async {});

    futures::join!(schedules, backups);

    Ok(())
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

    let desired = build_cnpg_backup(
        &name,
        &namespace,
        &cluster_name(&instance_name),
        backup.spec.method,
        &labels(&instance_name),
        owner,
    );

    let cnpg: Api<DynamicObject> = Api::namespaced_with(
        context.client.clone(),
        &namespace,
        &cnpg_backup_api_resource(),
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

    let status = status_from_cnpg(observed.as_ref(), &name, instance.as_ref());
    let backups: Api<IdentityInstanceBackup> = Api::namespaced(context.client.clone(), &namespace);
    patch_status(&backups, &name, json!({ "status": status })).await?;

    Ok(Action::requeue(REQUEUE))
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
        size_bytes: string("backupSize"),
        started_at: string("startedAt").map(|raw| Time(parse_time(&raw))),
        stopped_at: string("stoppedAt").map(|raw| Time(parse_time(&raw))),
        postgres_major: None,
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
