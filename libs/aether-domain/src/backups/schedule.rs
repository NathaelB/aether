//! When archives are taken, and how much history is kept.
//!
//! Two independent answers, the way [`crate::upgrades::policy`] splits what
//! may be applied from when it may be applied. A cadence nobody set is not the
//! same as a retention nobody set, and collapsing them would make one of them
//! inexpressible.
//!
//! Retention is evaluated here rather than left to the object store. A
//! lifecycle rule deletes without leaving anything behind to audit, and
//! "which archives were removed, when, and under what policy" is a question
//! that gets asked exactly once, at the worst possible time.

use std::{cmp::Reverse, fmt, num::NonZeroU32};

use chrono::{DateTime, Duration, NaiveTime, Utc, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    CoreError,
    backups::backup::{Backup, BackupMethod},
    deployments::DeploymentId,
    organisation::OrganisationId,
};

/// How often an archive is taken.
///
/// A closed set rather than a cron expression. A customer writing their own
/// cron can write one that fires every minute, and the platform would carry
/// out the request until the bill arrived. These two cover what anybody
/// actually asks for, and anything else is a conversation rather than a form
/// field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "every")]
pub enum Cadence {
    Daily {
        #[schema(value_type = String, example = "02:30")]
        at: NaiveTime,
    },
    Weekly {
        #[schema(value_type = String, example = "Sun")]
        day: Weekday,
        #[schema(value_type = String, example = "02:30")]
        at: NaiveTime,
    },
}

impl Cadence {
    /// The cron expression a Postgres operator is handed.
    ///
    /// Six fields with seconds first, and a `CRON_TZ` prefix, which is the
    /// dialect CloudNativePG's scheduler parses. The zone travels with the
    /// expression on purpose: emitting a UTC time computed once would be
    /// wrong for half of every year in any zone that observes daylight saving,
    /// and wrong in the direction of running the backup during the working day.
    pub fn to_cron(&self, zone: Tz) -> String {
        match self {
            Self::Daily { at } => format!(
                "CRON_TZ={zone} 0 {} {} * * *",
                at.format("%-M"),
                at.format("%-H")
            ),
            Self::Weekly { day, at } => format!(
                "CRON_TZ={zone} 0 {} {} * * {}",
                at.format("%-M"),
                at.format("%-H"),
                day.num_days_from_sunday()
            ),
        }
    }
}

/// How much history is kept.
///
/// The two rules are a union of what is **kept**, never an intersection. An
/// installation asking to keep the last 7 and anything from the last 30 days
/// keeps whichever is more, and a deployment that was quiet for a month still
/// has its seven archives. Read as an intersection, the same two numbers would
/// delete everything the moment a deployment stopped being backed up, which is
/// precisely when its history matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct Retention {
    /// Never fewer than this many, whatever their age.
    ///
    /// Non zero, so there is no way to express a policy that deletes the last
    /// archive a deployment has.
    #[schema(value_type = u32, example = 7)]
    keep_last: NonZeroU32,

    /// And everything younger than this, however many that is.
    #[schema(value_type = i64, example = 30)]
    keep_for_days: i64,
}

impl Retention {
    pub fn new(keep_last: NonZeroU32, keep_for_days: i64) -> Result<Self, CoreError> {
        if keep_for_days < 0 {
            return Err(CoreError::InvalidRetention {
                reason: format!("a window of {keep_for_days} days runs backwards"),
            });
        }

        Ok(Self {
            keep_last,
            keep_for_days,
        })
    }

    pub fn keep_last(&self) -> NonZeroU32 {
        self.keep_last
    }

    pub fn keep_for_days(&self) -> i64 {
        self.keep_for_days
    }

    /// Which of these archives the policy no longer requires.
    ///
    /// Takes the whole set rather than one archive at a time, because "is this
    /// one of the newest seven" cannot be answered about an archive on its own.
    /// Sorts rather than trusting the caller's order: a repository that changes
    /// its `ORDER BY` should not silently change which archives get deleted.
    pub fn expired<'a>(&self, backups: &'a [Backup], now: DateTime<Utc>) -> Vec<&'a Backup> {
        let mut newest_first: Vec<&Backup> = backups.iter().collect();
        newest_first.sort_by_key(|backup| Reverse(backup.finished_at));

        let floor = now - Duration::days(self.keep_for_days);
        let keep_last = self.keep_last.get() as usize;

        newest_first
            .into_iter()
            .enumerate()
            .filter(|(rank, backup)| *rank >= keep_last && backup.finished_at < floor)
            .map(|(_, backup)| backup)
            .collect()
    }
}

impl fmt::Display for Retention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the last {} and anything from the last {} days",
            self.keep_last, self.keep_for_days
        )
    }
}

/// What a deployment has asked the platform to do about its data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct BackupSchedule {
    pub deployment_id: DeploymentId,
    pub organisation_id: OrganisationId,

    pub cadence: Cadence,

    /// The zone the cadence is read in. Carried here rather than borrowed from
    /// the organisation, for the same reason
    /// [`crate::upgrades::policy::MaintenanceWindow`] carries its own: a
    /// customer running deployments on two continents wants two answers.
    #[schema(value_type = String, example = "Europe/Paris")]
    pub zone: Tz,

    pub retention: Retention,
    pub method: BackupMethod,

    /// Whether the platform acts on it. A schedule that is off keeps its
    /// settings, so turning backups back on does not mean filling the form in
    /// again from memory.
    pub enabled: bool,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BackupSchedule {
    /// What a deployment gets when nobody has said anything.
    ///
    /// On, nightly, and keeping a week. A platform whose default was off would
    /// be one where the first thing anybody learns about backups is that they
    /// did not have any.
    pub fn default_for(
        deployment_id: DeploymentId,
        organisation_id: OrganisationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            deployment_id,
            organisation_id,
            cadence: Cadence::Daily {
                at: NaiveTime::from_hms_opt(2, 30, 0).expect("02:30 is a time"),
            },
            zone: Tz::UTC,
            retention: Retention::new(NonZeroU32::new(7).expect("7 is not zero"), 30)
                .expect("30 days runs forwards"),
            method: BackupMethod::Physical,
            enabled: true,
            created_at: at,
            updated_at: at,
        }
    }

    pub fn to_cron(&self) -> String {
        self.cadence.to_cron(self.zone)
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::backups::backup::fixtures::backup;

    fn retention(keep_last: u32, keep_for_days: i64) -> Retention {
        Retention::new(NonZeroU32::new(keep_last).unwrap(), keep_for_days).unwrap()
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 12, 12, 0, 0).unwrap()
    }

    /// A set of archives, the newest taken `0` days ago.
    fn aged(days: &[i64]) -> Vec<Backup> {
        days.iter()
            .enumerate()
            .map(|(index, ago)| {
                let mut backup = backup(BackupMethod::Physical, "26.0.0", 17);
                backup.id =
                    crate::backups::backup::BackupId(uuid::Uuid::from_u128(index as u128 + 100));
                backup.finished_at = now() - Duration::days(*ago);
                backup
            })
            .collect()
    }

    #[test]
    fn nothing_expires_while_the_count_still_wants_it() {
        let backups = aged(&[100, 200, 300]);

        // All three are far older than the window, and all three are among the
        // newest seven. The count wins, and it has to: this is a deployment
        // whose backups stopped, which is when its history matters most.
        assert!(retention(7, 30).expired(&backups, now()).is_empty());
    }

    #[test]
    fn nothing_expires_while_the_window_still_wants_it() {
        let backups = aged(&[0, 1, 2, 3, 4]);

        // Five archives against a policy keeping the last one, but every one of
        // them is inside the window.
        assert!(retention(1, 30).expired(&backups, now()).is_empty());
    }

    #[test]
    fn an_archive_outside_both_rules_expires() {
        let backups = aged(&[0, 1, 60]);

        let expired = retention(2, 30).expired(&backups, now());

        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].finished_at, now() - Duration::days(60));
    }

    #[test]
    fn the_last_archive_is_never_expired() {
        let backups = aged(&[3650]);

        // Ten years old, and the only one there is. `keep_last` cannot be zero,
        // so no policy can ask for this one to go.
        assert!(retention(1, 1).expired(&backups, now()).is_empty());
    }

    #[test]
    fn expiry_does_not_depend_on_the_order_it_was_handed() {
        let ascending = aged(&[0, 1, 2, 90, 120]);
        let mut descending = ascending.clone();
        descending.reverse();

        let from_ascending: Vec<_> = retention(2, 30)
            .expired(&ascending, now())
            .iter()
            .map(|backup| backup.finished_at)
            .collect();
        let from_descending: Vec<_> = retention(2, 30)
            .expired(&descending, now())
            .iter()
            .map(|backup| backup.finished_at)
            .collect();

        assert_eq!(from_ascending, from_descending);
        assert_eq!(from_ascending.len(), 2);
    }

    #[test]
    fn a_window_that_runs_backwards_is_refused() {
        assert!(Retention::new(NonZeroU32::new(1).unwrap(), -1).is_err());
    }

    #[test]
    fn a_daily_cadence_carries_its_zone_rather_than_a_converted_hour() {
        let cadence = Cadence::Daily {
            at: NaiveTime::from_hms_opt(2, 30, 0).unwrap(),
        };

        assert_eq!(
            cadence.to_cron(Tz::Europe__Paris),
            "CRON_TZ=Europe/Paris 0 30 2 * * *"
        );
    }

    #[test]
    fn a_weekly_cadence_names_the_day() {
        let cadence = Cadence::Weekly {
            day: Weekday::Sun,
            at: NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
        };

        assert_eq!(cadence.to_cron(Tz::UTC), "CRON_TZ=UTC 0 0 3 * * 0");
    }

    #[test]
    fn a_deployment_nobody_configured_is_still_backed_up() {
        let schedule = BackupSchedule::default_for(
            DeploymentId(uuid::Uuid::from_u128(2)),
            OrganisationId(uuid::Uuid::from_u128(1)),
            now(),
        );

        assert!(schedule.enabled);
        assert_eq!(schedule.retention.keep_last().get(), 7);
    }
}
