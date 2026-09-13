use std::num::NonZeroU32;

use aether_auth::Identity;
use aether_core::{
    backups::{
        Backup, BackupSchedule, Cadence, Retention, commands::SetBackupScheduleCommand,
        ports::BackupService,
    },
    deployments::DeploymentId,
    organisation::OrganisationId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use chrono::{NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/backups")]
pub struct BackupsRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/backup-schedule")]
pub struct BackupScheduleRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListBackupsResponse {
    pub data: Vec<Backup>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct BackupScheduleResponse {
    pub data: BackupSchedule,
}

/// When a deployment is archived, and how much history is kept.
///
/// Read as a whole rather than field by field. A partial update would leave a
/// screen unable to say what the deployment is actually on without reading
/// back what it did not send, and the two would drift the first time a request
/// was lost.
#[derive(Deserialize, ToSchema)]
pub struct SetBackupScheduleRequest {
    /// `daily` or `weekly`.
    pub every: String,

    /// `HH:MM`, read in the zone below.
    #[schema(example = "02:30")]
    pub at: String,

    /// Required for a weekly cadence, ignored for a daily one.
    #[serde(default)]
    #[schema(example = "Sun")]
    pub day: Option<String>,

    /// A real zone rather than an offset: 02:30 local means 02:30 after a
    /// daylight saving change too, and an offset cannot say that.
    #[schema(example = "Europe/Paris")]
    pub zone: String,

    /// Never fewer than this many archives, whatever their age.
    #[schema(example = 7)]
    pub keep_last: u32,

    /// And everything younger than this, however many that is. The two are a
    /// union of what is kept, never an intersection.
    #[schema(example = 30)]
    pub keep_for_days: i64,

    pub enabled: bool,
}

impl SetBackupScheduleRequest {
    /// Reads the request as a schedule, naming whichever field could not be
    /// read rather than refusing as a whole.
    fn into_command(
        self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<SetBackupScheduleCommand, ApiError> {
        let refused = |reason: String| ApiError::BadRequest { reason };

        let at = NaiveTime::parse_from_str(&self.at, "%H:%M")
            .map_err(|_| refused(format!("'{}' is not a time of day: use HH:MM", self.at)))?;

        let cadence = match self.every.as_str() {
            "daily" => Cadence::Daily { at },
            "weekly" => {
                let day = self.day.as_deref().ok_or_else(|| {
                    refused("a weekly schedule needs the day it runs on".to_string())
                })?;

                Cadence::Weekly {
                    day: day.parse::<Weekday>().map_err(|_| {
                        refused(format!("'{day}' is not a day of the week: use Mon to Sun"))
                    })?,
                    at,
                }
            }
            other => {
                return Err(refused(format!(
                    "'{other}' is not a cadence: use daily or weekly"
                )));
            }
        };

        let zone = self
            .zone
            .parse()
            .map_err(|_| refused(format!("'{}' is not a time zone", self.zone)))?;

        // Zero is refused by the type rather than stored: there is no way to
        // express a policy that deletes the last archive a deployment has.
        let keep_last = NonZeroU32::new(self.keep_last).ok_or_else(|| {
            refused("keeping zero archives is not a retention policy".to_string())
        })?;

        Ok(SetBackupScheduleCommand {
            organisation_id,
            deployment_id,
            cadence,
            zone,
            retention: Retention::new(keep_last, self.keep_for_days)
                .map_err(|error| refused(error.to_string()))?,
            enabled: self.enabled,
        })
    }
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}/backups",
    summary = "list the archives taken of a deployment",
    tag = "deployments",
    description = "Newest first. Only archives that exist are listed: an attempt that \
                   produced nothing is in the audit trail, not here.",
    params(BackupsRoute),
    responses(
        (status = 200, description = "The archives this deployment has", body = ListBackupsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not see this deployment's archives", body = ApiError),
        (status = 404, description = "No such deployment", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_backups_handler(
    BackupsRoute {
        organisation_id,
        deployment_id,
    }: BackupsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListBackupsResponse>, ApiError> {
    let backups = state
        .service
        .list_backups(
            identity,
            OrganisationId(organisation_id),
            DeploymentId(deployment_id),
        )
        .await?;

    Ok(Response::OK(ListBackupsResponse { data: backups }))
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}/backup-schedule",
    summary = "read when a deployment is archived",
    tag = "deployments",
    description = "Answers with the platform default when nobody has changed it, because \
                   that is what the deployment is actually on.",
    params(BackupScheduleRoute),
    responses(
        (status = 200, description = "When this deployment is archived", body = BackupScheduleResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not see this deployment's archives", body = ApiError),
        (status = 404, description = "No such deployment", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_backup_schedule_handler(
    BackupScheduleRoute {
        organisation_id,
        deployment_id,
    }: BackupScheduleRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<BackupScheduleResponse>, ApiError> {
    let schedule = state
        .service
        .get_backup_schedule(
            identity,
            OrganisationId(organisation_id),
            DeploymentId(deployment_id),
        )
        .await?;

    Ok(Response::OK(BackupScheduleResponse { data: schedule }))
}

#[utoipa::path(
    put,
    path = "/{organisation_id}/deployments/{deployment_id}/backup-schedule",
    summary = "change when a deployment is archived",
    tag = "deployments",
    description = "Answers with what was stored rather than what was asked for, so a screen \
                   draws the schedule that applies instead of the one it requested.",
    params(BackupScheduleRoute),
    request_body = SetBackupScheduleRequest,
    responses(
        (status = 200, description = "The schedule that now applies", body = BackupScheduleResponse),
        (status = 400, description = "The request does not describe a schedule", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not change this deployment's archives", body = ApiError),
        (status = 404, description = "No such deployment", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn set_backup_schedule_handler(
    BackupScheduleRoute {
        organisation_id,
        deployment_id,
    }: BackupScheduleRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<SetBackupScheduleRequest>,
) -> Result<Response<BackupScheduleResponse>, ApiError> {
    let command =
        request.into_command(OrganisationId(organisation_id), DeploymentId(deployment_id))?;

    let schedule = state.service.set_backup_schedule(identity, command).await?;

    Ok(Response::OK(BackupScheduleResponse { data: schedule }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daily() -> SetBackupScheduleRequest {
        SetBackupScheduleRequest {
            every: "daily".to_string(),
            at: "02:30".to_string(),
            day: None,
            zone: "Europe/Paris".to_string(),
            keep_last: 7,
            keep_for_days: 30,
            enabled: true,
        }
    }

    fn command(request: SetBackupScheduleRequest) -> Result<SetBackupScheduleCommand, ApiError> {
        request.into_command(
            OrganisationId(Uuid::from_u128(1)),
            DeploymentId(Uuid::from_u128(2)),
        )
    }

    #[test]
    fn a_daily_schedule_is_read_in_its_own_zone() {
        let read = command(daily()).expect("a daily schedule");

        assert_eq!(
            read.cadence,
            Cadence::Daily {
                at: NaiveTime::from_hms_opt(2, 30, 0).unwrap()
            }
        );
        assert_eq!(read.zone.name(), "Europe/Paris");
    }

    #[test]
    fn a_weekly_schedule_needs_the_day_it_runs_on() {
        let missing = SetBackupScheduleRequest {
            every: "weekly".to_string(),
            day: None,
            ..daily()
        };

        assert!(command(missing).is_err());

        let named = SetBackupScheduleRequest {
            every: "weekly".to_string(),
            day: Some("Sun".to_string()),
            ..daily()
        };

        assert_eq!(
            command(named).expect("a weekly schedule").cadence,
            Cadence::Weekly {
                day: Weekday::Sun,
                at: NaiveTime::from_hms_opt(2, 30, 0).unwrap()
            }
        );
    }

    /// Every refusal names the field. A caller looking at their own form needs
    /// to know which line to fix.
    #[test]
    fn a_value_nobody_can_read_is_named_rather_than_defaulted() {
        for (request, expected) in [
            (
                SetBackupScheduleRequest {
                    at: "half past two".to_string(),
                    ..daily()
                },
                "half past two",
            ),
            (
                SetBackupScheduleRequest {
                    zone: "Middle-earth/Shire".to_string(),
                    ..daily()
                },
                "Middle-earth/Shire",
            ),
            (
                SetBackupScheduleRequest {
                    every: "hourly".to_string(),
                    ..daily()
                },
                "hourly",
            ),
        ] {
            let Err(ApiError::BadRequest { reason }) = command(request) else {
                panic!("'{expected}' was accepted");
            };

            assert!(reason.contains(expected), "got {reason}");
        }
    }

    /// There is no way to express a policy that deletes the last archive a
    /// deployment has, and the refusal says so rather than storing a zero.
    #[test]
    fn keeping_nothing_is_not_a_retention_policy() {
        let nothing = SetBackupScheduleRequest {
            keep_last: 0,
            ..daily()
        };

        assert!(command(nothing).is_err());
    }

    #[test]
    fn a_window_that_runs_backwards_is_refused() {
        let backwards = SetBackupScheduleRequest {
            keep_for_days: -1,
            ..daily()
        };

        assert!(command(backwards).is_err());
    }
}
