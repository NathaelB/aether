//! What a customer delegates, and when the platform may act on it.
//!
//! Two separate answers. The policy says which steps may be applied without
//! asking; the window says when. Neither implies the other, and a deployment
//! that answers yes to one and no to the other is upgraded by hand.

use chrono::{
    DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc, Weekday,
};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::version::VersionChange;

/// How much of upgrading this deployment the customer hands over.
///
/// Named for what it governs. It decides nothing about an upgrade somebody
/// asks for by hand, which is why it is not simply called the upgrade policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutoUpgradePolicy {
    /// Nothing is applied without being asked for. The default, and what every
    /// deployment that predates this setting gets: inheriting an automation
    /// nobody chose is not a safe default.
    #[default]
    Manual,
    Patch,
    PatchAndMinor,
}

impl AutoUpgradePolicy {
    /// Whether this step may be applied without asking.
    ///
    /// A major never may, under any policy. That is not a setting with a
    /// cautious default, it is an invariant: the enum cannot express the
    /// alternative, so no configuration screen and no later refactor can
    /// introduce it by accident.
    pub fn allows(self, change: VersionChange) -> bool {
        match (self, change) {
            (_, VersionChange::Major) => false,
            (Self::Manual, _) => false,
            (Self::Patch, VersionChange::Patch) => true,
            (Self::Patch, VersionChange::Minor) => false,
            (Self::PatchAndMinor, _) => true,
        }
    }
}

/// When the platform may act on its own.
///
/// Carries its own zone rather than borrowing the organisation's. A customer
/// running deployments on two continents wants two windows, and a single zone
/// per organisation would make one of them wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MaintenanceWindow {
    /// The local day the window opens on. A window that runs past midnight
    /// finishes on the following day; it does not open twice.
    #[schema(value_type = String, example = "Sun")]
    pub day: Weekday,
    #[schema(value_type = String, example = "03:00")]
    pub start: NaiveTime,
    #[schema(value_type = i64, example = 120)]
    pub duration: Duration,
    #[schema(value_type = String, example = "Europe/Paris")]
    pub timezone: Tz,
}

impl MaintenanceWindow {
    /// Refuses a window that cannot close.
    ///
    /// A full week is the limit: at that point it is not a window, it is
    /// permission to act at any time, and saying so explicitly is better than
    /// a setting that reads as a restriction and is not one.
    pub fn new(
        day: Weekday,
        start: NaiveTime,
        duration: Duration,
        timezone: Tz,
    ) -> Result<Self, MaintenanceWindowError> {
        if duration <= Duration::zero() {
            return Err(MaintenanceWindowError::NotPositive);
        }
        if duration >= Duration::days(7) {
            return Err(MaintenanceWindowError::LongerThanAWeek);
        }

        Ok(Self {
            day,
            start,
            duration,
            timezone,
        })
    }

    /// Whether `at` falls inside the window.
    ///
    /// The window is anchored to the instant it opens and then lasts exactly
    /// its duration. Reading the local wall clock instead looks simpler and is
    /// wrong twice a year: on the night an hour repeats, 02:30 comes round
    /// again and the window opens a second time, which is how an upgrade the
    /// customer allowed once gets applied twice.
    pub fn contains(&self, at: DateTime<Utc>) -> bool {
        self.elapsed_since_open(at).is_some()
    }

    /// How much of the window is left at `at`, or `None` if it is not open.
    pub fn remaining_at(&self, at: DateTime<Utc>) -> Option<Duration> {
        self.elapsed_since_open(at)
            .map(|elapsed| self.duration - elapsed)
    }

    /// Whether something taking `estimated` can start now and still finish
    /// inside the window.
    ///
    /// Starting what cannot finish is the failure this exists to prevent: a
    /// rolling upgrade interrupted by the end of a window is not paused, it is
    /// a half-upgraded instance in working hours.
    pub fn fits(&self, at: DateTime<Utc>, estimated: Duration) -> bool {
        self.remaining_at(at)
            .is_some_and(|remaining| remaining >= estimated)
    }

    /// How long the window has been open at `at`, or `None` if it is not.
    fn elapsed_since_open(&self, at: DateTime<Utc>) -> Option<Duration> {
        let today = at.with_timezone(&self.timezone).date_naive();

        // Far enough back to catch a window that opened days ago and is still
        // running, plus one day of slack for the local date being ahead of UTC.
        for back in 0..=self.duration.num_days() + 1 {
            let date = today - Duration::days(back);
            if date.weekday() != self.day {
                continue;
            }

            let Some(opened) = self.opens_on(date) else {
                continue;
            };

            if at >= opened && at < opened + self.duration {
                return Some(at - opened);
            }
        }

        None
    }

    /// The instant the window opens on a given local date.
    ///
    /// Two irregular nights a year have to be answered for. When the clock
    /// jumps forward the written time may not exist at all, and the window
    /// opens when the clock next reaches it rather than not opening. When it
    /// goes back the written time happens twice, and the window opens on the
    /// first, because a customer who allowed one window a week meant one.
    fn opens_on(&self, date: NaiveDate) -> Option<DateTime<Utc>> {
        // Walking forward a minute at a time over a gap: the jump is an hour
        // in every zone that has one, and searching is simpler to read than
        // reconstructing the transition from the offsets either side of it.
        for minute in 0..(3 * 60) {
            let local = date.and_time(self.start) + Duration::minutes(minute);

            match self.timezone.from_local_datetime(&local) {
                LocalResult::Single(moment) => return Some(moment.with_timezone(&Utc)),
                LocalResult::Ambiguous(first, _) => return Some(first.with_timezone(&Utc)),
                LocalResult::None => continue,
            }
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MaintenanceWindowError {
    #[error("a maintenance window must be longer than nothing")]
    NotPositive,

    #[error("a maintenance window of a week or more is not a window")]
    LongerThanAWeek,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    fn window(day: Weekday, start: &str, hours: i64, tz: Tz) -> MaintenanceWindow {
        MaintenanceWindow::new(
            day,
            NaiveTime::parse_from_str(start, "%H:%M").expect("a valid time"),
            Duration::hours(hours),
            tz,
        )
        .expect("a valid window")
    }

    #[test]
    fn a_major_is_never_automatic() {
        for policy in [
            AutoUpgradePolicy::Manual,
            AutoUpgradePolicy::Patch,
            AutoUpgradePolicy::PatchAndMinor,
        ] {
            assert!(
                !policy.allows(VersionChange::Major),
                "{policy:?} must not apply a major on its own"
            );
        }
    }

    #[test]
    fn each_policy_allows_exactly_what_it_says() {
        assert!(!AutoUpgradePolicy::Manual.allows(VersionChange::Patch));
        assert!(!AutoUpgradePolicy::Manual.allows(VersionChange::Minor));

        assert!(AutoUpgradePolicy::Patch.allows(VersionChange::Patch));
        assert!(!AutoUpgradePolicy::Patch.allows(VersionChange::Minor));

        assert!(AutoUpgradePolicy::PatchAndMinor.allows(VersionChange::Patch));
        assert!(AutoUpgradePolicy::PatchAndMinor.allows(VersionChange::Minor));
    }

    /// A deployment that predates the setting inherits no automation. Anything
    /// else would apply upgrades nobody asked for to every existing customer.
    #[test]
    fn the_default_delegates_nothing() {
        assert_eq!(AutoUpgradePolicy::default(), AutoUpgradePolicy::Manual);
    }

    #[test]
    fn a_window_must_be_able_to_close() {
        let time = NaiveTime::parse_from_str("03:00", "%H:%M").expect("valid");

        assert!(matches!(
            MaintenanceWindow::new(Weekday::Sun, time, Duration::zero(), Tz::UTC),
            Err(MaintenanceWindowError::NotPositive)
        ));
        assert!(matches!(
            MaintenanceWindow::new(Weekday::Sun, time, Duration::days(7), Tz::UTC),
            Err(MaintenanceWindowError::LongerThanAWeek)
        ));
    }

    #[test]
    fn an_instant_inside_the_window_is_inside_it() {
        let window = window(Weekday::Sun, "03:00", 2, Tz::UTC);

        assert!(window.contains(at("2026-01-04T03:00:00Z")));
        assert!(window.contains(at("2026-01-04T04:59:59Z")));
    }

    /// The end is exclusive. An upgrade starting at the closing instant has no
    /// time at all, and calling that "inside" is how one gets started anyway.
    #[test]
    fn the_closing_instant_is_outside() {
        let window = window(Weekday::Sun, "03:00", 2, Tz::UTC);

        assert!(!window.contains(at("2026-01-04T05:00:00Z")));
        assert!(!window.contains(at("2026-01-04T02:59:59Z")));
    }

    #[test]
    fn another_day_is_outside() {
        let window = window(Weekday::Sun, "03:00", 2, Tz::UTC);

        assert!(!window.contains(at("2026-01-05T03:30:00Z")));
    }

    /// A window that runs past midnight finishes on the following day. It does
    /// not open a second time on that day.
    #[test]
    fn a_window_crossing_midnight_stays_one_window() {
        let window = window(Weekday::Sun, "23:00", 4, Tz::UTC);

        assert!(window.contains(at("2026-01-04T23:30:00Z")), "sunday night");
        assert!(
            window.contains(at("2026-01-05T02:59:00Z")),
            "monday small hours"
        );
        assert!(
            !window.contains(at("2026-01-05T03:00:00Z")),
            "monday, closed"
        );
        assert!(
            !window.contains(at("2026-01-05T23:30:00Z")),
            "monday night is not a second opening"
        );
    }

    /// The window is written in local time, so it stays at 03:00 local across
    /// a daylight saving change rather than drifting to 02:00 or 04:00.
    #[test]
    fn the_window_follows_the_local_clock_across_a_daylight_saving_change() {
        let window = window(Weekday::Sun, "03:00", 2, Tz::Europe__Paris);

        // Winter: Paris is UTC+1, so 03:00 local is 02:00 UTC.
        assert!(window.contains(at("2026-01-04T02:30:00Z")));
        assert!(!window.contains(at("2026-01-04T01:30:00Z")));

        // Summer: Paris is UTC+2, so 03:00 local is 01:00 UTC.
        assert!(window.contains(at("2026-07-05T01:30:00Z")));
        assert!(
            !window.contains(at("2026-07-05T03:30:00Z")),
            "05:30 local, closed"
        );
    }

    /// Spring forward skips an hour of local time. The window opens once, and
    /// is simply shorter in real time, which is what someone who wrote "03:00
    /// for two hours" expects. What must not happen is it never opening.
    #[test]
    fn the_window_still_opens_on_the_night_an_hour_goes_missing() {
        // Paris skips 02:00 to 03:00 on 2026-03-29.
        let window = window(Weekday::Sun, "02:30", 2, Tz::Europe__Paris);
        let mut opened = false;

        for minute in 0..(6 * 60) {
            let moment = at("2026-03-29T00:00:00Z") + Duration::minutes(minute);
            if window.contains(moment) {
                opened = true;
                break;
            }
        }

        assert!(opened, "the window never opened on the short night");
    }

    /// Autumn repeats an hour. The window must not be entered twice: it is one
    /// window that happens to last an hour longer in real time.
    #[test]
    fn the_repeated_hour_does_not_open_the_window_twice() {
        // Paris repeats 02:00 to 03:00 on 2026-10-25.
        let window = window(Weekday::Sun, "02:30", 1, Tz::Europe__Paris);

        let mut runs = 0;
        let mut was_open = false;
        for minute in 0..(8 * 60) {
            let moment = at("2026-10-24T22:00:00Z") + Duration::minutes(minute);
            let open = window.contains(moment);
            if open && !was_open {
                runs += 1;
            }
            was_open = open;
        }

        assert_eq!(runs, 1, "the window opened {runs} times, not once");
    }

    #[test]
    fn what_is_left_shrinks_as_the_window_runs() {
        let window = window(Weekday::Sun, "03:00", 2, Tz::UTC);

        assert_eq!(
            window.remaining_at(at("2026-01-04T03:00:00Z")),
            Some(Duration::hours(2))
        );
        assert_eq!(
            window.remaining_at(at("2026-01-04T04:30:00Z")),
            Some(Duration::minutes(30))
        );
        assert_eq!(window.remaining_at(at("2026-01-04T06:00:00Z")), None);
    }

    /// The rule the scheduler leans on. A rolling upgrade interrupted by the
    /// end of a window is not paused, it is a half-upgraded instance in
    /// working hours.
    #[test]
    fn what_cannot_finish_in_time_does_not_fit() {
        let window = window(Weekday::Sun, "03:00", 2, Tz::UTC);
        let half_past_four = at("2026-01-04T04:30:00Z");

        assert!(window.fits(half_past_four, Duration::minutes(30)));
        assert!(!window.fits(half_past_four, Duration::minutes(31)));
    }

    #[test]
    fn nothing_fits_outside_the_window() {
        let window = window(Weekday::Sun, "03:00", 2, Tz::UTC);

        assert!(!window.fits(at("2026-01-05T03:30:00Z"), Duration::seconds(1)));
    }

    #[test]
    fn a_window_serialises_in_the_same_case_as_the_rest_of_the_api() {
        assert_eq!(
            serde_json::to_string(&AutoUpgradePolicy::PatchAndMinor).expect("serialises"),
            "\"patch_and_minor\""
        );
    }
}
