use aether_auth::Identity;
use aether_core::{
    backups::{
        BackupMethod, PostgresMajor,
        commands::{RecordArchiveCommand, RecordArchiveFailureCommand},
        keys::{KeyName, KeyRef, KeyVersion, ProviderName},
        ports::BackupService,
    },
    dataplane::value_objects::DataPlaneId,
    deployments::DeploymentId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/deployments/{deployment_id}/archive")]
pub struct ReportArchiveRoute {
    pub dataplane_id: DataPlaneId,
    pub deployment_id: DeploymentId,
}

/// What a data plane says about one archive.
///
/// Either it exists, and every field describing it is present, or it does not
/// and only `error` is. The two are the same endpoint because they are the two
/// outcomes of one attempt, and they are told apart by which fields arrived
/// rather than by a flag somebody can set inconsistently.
#[derive(Deserialize, ToSchema)]
pub struct ReportArchiveRequest {
    /// Why no archive exists. Present only when none does.
    ///
    /// A failure is recorded in the audit trail and nowhere else: there is no
    /// such thing as a backup that failed, only an attempt that was made.
    #[serde(default)]
    pub error: Option<String>,

    /// Where the archive sits, relative to the deployment's own prefix.
    ///
    /// Relative on purpose. An absolute path would be a second way to address
    /// an archive, and the control plane would have to trust a data plane not
    /// to name another tenant's.
    #[serde(default)]
    pub object_key: Option<String>,

    /// `logical` or `physical`.
    #[serde(default)]
    pub method: Option<String>,

    #[serde(default)]
    pub postgres_major: Option<u32>,

    /// Which key wrapped the data key: the provider, the key, and the version.
    #[serde(default)]
    pub key_provider: Option<String>,
    #[serde(default)]
    pub key_name: Option<String>,
    #[serde(default)]
    pub key_version: Option<u32>,

    /// A string rather than a number, because a size passes through a signed
    /// 32 bit integer somewhere in every Kubernetes toolchain and a 3 GB
    /// archive read back as a negative number is worse than one nobody can
    /// sum.
    #[serde(default)]
    pub size_bytes: Option<String>,

    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReportArchiveResponseData {
    /// The archive's id, when one was recorded. Absent for a reported failure.
    pub backup_id: Option<String>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ReportArchiveResponse {
    data: ReportArchiveResponseData,
}

impl ReportArchiveRequest {
    /// Reads the report as an archive that exists.
    ///
    /// Every missing field is named rather than defaulted. A report the
    /// control plane cannot fully understand describes an archive it would
    /// later offer as a restore, and guessing at a field is how an archive
    /// nobody can read gets recorded as one that can.
    fn into_archive(
        self,
        dataplane_id: DataPlaneId,
        deployment_id: DeploymentId,
    ) -> Result<RecordArchiveCommand, String> {
        let required = |name: &str| format!("{name} is required when an archive exists");

        let size_bytes: u64 = self
            .size_bytes
            .ok_or_else(|| required("size_bytes"))?
            .parse()
            .map_err(|_| "size_bytes is not a number of bytes".to_string())?;

        Ok(RecordArchiveCommand {
            dataplane_id,
            deployment_id,
            object_key: self.object_key.ok_or_else(|| required("object_key"))?,
            method: BackupMethod::try_from(self.method.ok_or_else(|| required("method"))?.as_str())
                .map_err(|error| error.to_string())?,
            postgres_major: PostgresMajor(
                self.postgres_major
                    .ok_or_else(|| required("postgres_major"))?,
            ),
            key: KeyRef::new(
                ProviderName::new(self.key_provider.ok_or_else(|| required("key_provider"))?),
                KeyName::new(self.key_name.ok_or_else(|| required("key_name"))?)
                    .map_err(|error| error.to_string())?,
                KeyVersion::new(self.key_version.ok_or_else(|| required("key_version"))?),
            ),
            size_bytes,
            started_at: self.started_at.ok_or_else(|| required("started_at"))?,
            finished_at: self.finished_at.ok_or_else(|| required("finished_at"))?,
        })
    }
}

#[utoipa::path(
    post,
    path = "/{dataplane_id}/deployments/{deployment_id}/archive",
    summary = "report an archive a data plane took, or an attempt that produced none",
    tag = "dataplanes",
    description = "Records an archive the data plane wrote, or audits an attempt that \
                   produced none. Idempotent on the object key: reports arrive at least \
                   once, and an archive recorded twice would be counted twice by \
                   retention and offered twice as a restore.",
    params(ReportArchiveRoute),
    request_body = ReportArchiveRequest,
    responses(
        (status = 200, description = "Archive recorded", body = ReportArchiveResponse),
        (status = 400, description = "The report does not describe an archive", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Caller is not herald, or the deployment runs elsewhere", body = ApiError),
        (status = 500, description = "Internal error", body = ApiError),
    ),
    security(("bearer_auth" = []))
)]
pub async fn report_archive_handler(
    ReportArchiveRoute {
        dataplane_id,
        deployment_id,
    }: ReportArchiveRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<ReportArchiveRequest>,
) -> Result<Response<ReportArchiveResponse>, ApiError> {
    if let Some(reason) = request.error.clone() {
        state
            .service
            .record_archive_failure(
                identity,
                RecordArchiveFailureCommand {
                    dataplane_id,
                    deployment_id,
                    reason,
                    attempted_at: request.finished_at.unwrap_or_else(Utc::now),
                },
            )
            .await?;

        return Ok(Response::OK(ReportArchiveResponse {
            data: ReportArchiveResponseData { backup_id: None },
        }));
    }

    let command = request
        .into_archive(dataplane_id, deployment_id)
        .map_err(|reason| ApiError::BadRequest { reason })?;

    let backup = state.service.record_archive(identity, command).await?;

    Ok(Response::OK(ReportArchiveResponse {
        data: ReportArchiveResponseData {
            backup_id: Some(backup.id.to_string()),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_report() -> ReportArchiveRequest {
        ReportArchiveRequest {
            error: None,
            object_key: Some("base/20260912T0230Z/data.tar.gz".to_string()),
            method: Some("physical".to_string()),
            postgres_major: Some(17),
            key_provider: Some("platform".to_string()),
            key_name: Some("aether-backups".to_string()),
            key_version: Some(1),
            size_bytes: Some("4136598".to_string()),
            started_at: Some(Utc::now()),
            finished_at: Some(Utc::now()),
        }
    }

    fn ids() -> (DataPlaneId, DeploymentId) {
        (
            DataPlaneId(uuid::Uuid::new_v4()),
            DeploymentId(uuid::Uuid::new_v4()),
        )
    }

    #[test]
    fn a_complete_report_describes_an_archive() {
        let (dataplane, deployment) = ids();

        let command = full_report()
            .into_archive(dataplane, deployment)
            .expect("a complete report");

        assert_eq!(command.size_bytes, 4_136_598);
        assert_eq!(command.postgres_major.0, 17);
        assert_eq!(command.key.name.as_str(), "aether-backups");
    }

    /// Named rather than defaulted. A field guessed at here becomes an archive
    /// the platform offers as a restore and cannot actually read.
    #[test]
    fn every_missing_field_is_named() {
        let (dataplane, deployment) = ids();

        for (name, mutate) in [
            (
                "object_key",
                (|r: &mut ReportArchiveRequest| r.object_key = None)
                    as fn(&mut ReportArchiveRequest),
            ),
            ("method", |r: &mut ReportArchiveRequest| r.method = None),
            ("postgres_major", |r: &mut ReportArchiveRequest| {
                r.postgres_major = None
            }),
            ("key_provider", |r: &mut ReportArchiveRequest| {
                r.key_provider = None
            }),
            ("key_name", |r: &mut ReportArchiveRequest| r.key_name = None),
            ("key_version", |r: &mut ReportArchiveRequest| {
                r.key_version = None
            }),
            ("size_bytes", |r: &mut ReportArchiveRequest| {
                r.size_bytes = None
            }),
            ("started_at", |r: &mut ReportArchiveRequest| {
                r.started_at = None
            }),
            ("finished_at", |r: &mut ReportArchiveRequest| {
                r.finished_at = None
            }),
        ] {
            let mut report = full_report();
            mutate(&mut report);

            let refused = report
                .into_archive(dataplane, deployment)
                .expect_err("a report missing {name} was accepted");

            assert!(
                refused.contains(name),
                "the message does not say which field is missing: {refused}"
            );
        }
    }

    /// A size that overflowed somewhere in the toolchain is refused rather
    /// than recorded, which is why it travels as a string.
    #[test]
    fn a_size_that_is_not_a_number_of_bytes_is_refused() {
        let (dataplane, deployment) = ids();

        let refused = ReportArchiveRequest {
            size_bytes: Some("-1".to_string()),
            ..full_report()
        }
        .into_archive(dataplane, deployment)
        .expect_err("a negative size was accepted");

        assert!(refused.contains("size_bytes"));
    }
}
