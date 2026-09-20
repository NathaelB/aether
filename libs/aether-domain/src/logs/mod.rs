//! Reading an instance's logs without becoming the keeper of them.
//!
//! Logs hold identities, addresses and sometimes tokens. Pulling them into the
//! control plane's database would turn a platform that stores versions and
//! settings into one that stores every customer's personal data, with the
//! retention, export and deletion obligations that come with it.
//!
//! So nothing here is written down. A request opens a session, the data plane
//! pushes lines into it, and the lines travel straight out to whoever asked.
//! When the session ends they are gone. The only thing recorded is that
//! somebody looked, which is the opposite concern: reading another person's
//! logs must leave a trace.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{CoreError, deployments::DeploymentId};

pub mod commands;
pub mod ports;
pub mod service;

/// How far back a request may reach.
///
/// Enforced here rather than in the screen. A cap that only exists in the
/// console is not a cap: the endpoint is reachable without it, and the point
/// of the limit is to keep a single request from dragging a week of a busy
/// instance's logs through the control plane.
pub const MAX_WINDOW_MINUTES: i64 = 60;

/// A window that cannot be longer than the cap, because there is no way to
/// build one that is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, ToSchema)]
pub struct LogWindow(i64);

impl LogWindow {
    /// Refuses rather than quietly shortening. Someone who asked for a day
    /// and silently received an hour would read the gap as "nothing
    /// happened", which is the one thing logs must never be made to say.
    pub fn minutes(requested: i64) -> Result<Self, CoreError> {
        if requested <= 0 {
            return Err(CoreError::InvalidLogWindow {
                requested,
                max: MAX_WINDOW_MINUTES,
            });
        }

        if requested > MAX_WINDOW_MINUTES {
            return Err(CoreError::InvalidLogWindow {
                requested,
                max: MAX_WINDOW_MINUTES,
            });
        }

        Ok(Self(requested))
    }

    pub fn as_minutes(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct LogSessionId(pub Uuid);

impl std::fmt::Display for LogSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One line on its way through. Never stored, never aggregated, never indexed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LogLine {
    pub at: DateTime<Utc>,
    /// Which container it came from, so a customer running two can tell them
    /// apart.
    pub source: String,
    pub message: String,
}

/// Why a session is over.
///
/// Carried to the reader rather than left to a closed socket. A connection
/// that simply ends is indistinguishable from an instance that went quiet,
/// and a screen that cannot tell those apart shows the second as the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionEnd {
    /// The data plane stopped following: its own ceiling, or the pods ran
    /// out. Opening another session is the way to keep watching.
    Finished,
    /// The pods could not be read at all, so there was never anything to
    /// follow. Opening another session would fail the same way.
    Unreadable,
    /// Nothing was heard from the data plane for long enough to call it gone.
    /// Unlike the other two this is the control plane's own verdict: the data
    /// plane never said anything, which is the problem.
    Silent,
}

/// What comes out of an open session.
///
/// Three things rather than lines alone, because the two that are not lines
/// are what stop a quiet instance from reading as a dead one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Relayed {
    Line(LogLine),
    /// The data plane has nothing to send and is still following.
    ///
    /// Never shown to anybody. It exists so that silence from the instance
    /// and silence from the data plane are different facts, and only the
    /// second one ends the read.
    StillFollowing,
    Ended(SessionEnd),
}

/// An open read. Lives as long as the connection asking for it, and not a
/// moment longer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSession {
    pub id: LogSessionId,
    pub deployment_id: DeploymentId,
    pub window: LogWindow,
    pub opened_at: DateTime<Utc>,
}

/// The level a line whose format Herald could not read is given.
///
/// Matches `UNKNOWN_LEVEL` in `herald-core::domain::log_index` without
/// depending on that crate -- a control-plane crate reproducing one frozen
/// string constant is cheaper than a data-plane dependency for it.
pub const UNKNOWN_LEVEL: &str = "unknown";

/// The severity vocabulary `derive_level` (herald-core) writes into the
/// index, in the order #292 decided on: `trace < debug < info < warn < error
/// < fatal`.
///
/// [`UNKNOWN_LEVEL`] is deliberately not a variant here. It is not a
/// severity Herald observed -- it is Herald admitting a line's format did not
/// match anything it recognises -- so it has no place on a scale of how bad a
/// line is. [`LogLevel::and_above`] is the one place that matters: a floor
/// always includes it, so a search can never silently exclude exactly the
/// lines whose meaning nobody could read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogLevel {
    const ORDERED: [LogLevel; 6] = [
        LogLevel::Trace,
        LogLevel::Debug,
        LogLevel::Info,
        LogLevel::Warn,
        LogLevel::Error,
        LogLevel::Fatal,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
            LogLevel::Fatal => "fatal",
        }
    }

    /// Every level at or above this floor, as the index writes them, plus
    /// [`UNKNOWN_LEVEL`] -- always, not only when the floor is the lowest.
    /// Excluding it would hide exactly the lines that are disproportionately
    /// where something unusual is happening.
    pub fn and_above(self) -> Vec<&'static str> {
        let mut levels: Vec<&'static str> = Self::ORDERED
            .into_iter()
            .filter(|level| *level >= self)
            .map(LogLevel::as_str)
            .collect();
        levels.push(UNKNOWN_LEVEL);
        levels
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    /// A plain string rather than [`LogLevel`] itself at the API boundary --
    /// `level_floor` is read from a query string, where a caller sends text,
    /// not the variant `utoipa` would otherwise have to invent a schema
    /// reference for.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.to_ascii_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            "fatal" => Ok(LogLevel::Fatal),
            other => Err(format!(
                "'{other}' is not a log level (trace, debug, info, warn, error, fatal)"
            )),
        }
    }
}

/// How far back and how recent a search reaches, as an explicit range rather
/// than an offset from "now" the way the live tail's [`LogWindow`] is.
///
/// Anchoring to `Utc::now()` would mean the *same request*, asked twice,
/// covers two different spans -- which breaks the one guarantee this search
/// has to keep: that the same query returns the same answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct LogSearchWindow {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

impl LogSearchWindow {
    /// The index keeps 30 days (`docs/log-search-index.md`'s retention
    /// block); reaching further back than that can only ever answer
    /// "nothing", so the cap matches retention rather than inventing a
    /// second number that could drift from it.
    pub const MAX_SPAN_DAYS: i64 = 30;

    pub fn new(from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Self, CoreError> {
        if to <= from {
            return Err(CoreError::InvalidLogSearchWindow {
                reason: format!("the window ends ({to}) at or before it starts ({from})"),
            });
        }

        if to - from > chrono::Duration::days(Self::MAX_SPAN_DAYS) {
            return Err(CoreError::InvalidLogSearchWindow {
                reason: format!(
                    "a search cannot span more than {} days, the index's own retention",
                    Self::MAX_SPAN_DAYS
                ),
            });
        }

        Ok(Self { from, to })
    }
}

/// The translated request a search index adapter receives -- everything
/// [`commands::SearchLogsCommand`] carries except the tenant, which travels
/// as its own argument on [`ports::LogSearchIndex`] rather than as a field
/// here. Isolation is which index is even reachable, so the one thing that
/// picks it must never be something a filter value could carry instead.
#[derive(Debug, Clone, PartialEq)]
pub struct LogSearchFilter {
    pub deployment_id: Option<DeploymentId>,
    pub window: LogSearchWindow,
    pub level_floor: LogLevel,
    pub text: Option<String>,
}

/// One line the index matched.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct LogSearchHit {
    pub timestamp: DateTime<Utc>,
    pub deployment_id: DeploymentId,
    pub source: String,
    pub level: String,
    pub message: String,
}

/// How many hits a single search answers with, regardless of how many
/// matched. Not a caller-chosen page size -- the query shape is deliberately
/// just a time range, a level floor, free text and a deployment, nothing
/// resembling pagination -- but a fixed ceiling so one search cannot drag an
/// unbounded result set through the control plane.
pub const MAX_SEARCH_HITS: usize = 200;

/// How many distinct values a single facet reports, regardless of how many
/// actually occur. An organisation with a thousand containers must not hand
/// the console a thousand `source` buckets.
pub const MAX_FACET_TERMS: usize = 20;

/// One distinct value a facet found, and how many matching lines carried it.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct LogFacetBucket {
    pub value: String,
    pub count: u64,
}

/// Counts per distinct value across the whole matching set, not just the
/// [`MAX_SEARCH_HITS`] hits returned alongside them -- a count derived from
/// the capped page would be wrong the moment a query matches more than that.
///
/// Value counts, not coverage percentages: the doc mapping has six fixed
/// fields that every line carries, so a percentage would read 100% on every
/// facet and say nothing.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct LogFacets {
    pub level: Vec<LogFacetBucket>,
    pub source: Vec<LogFacetBucket>,
    pub deployment_id: Vec<LogFacetBucket>,
}

/// What a search answers with: up to [`MAX_SEARCH_HITS`] hits, how many
/// actually matched so a caller can tell a complete answer from a capped one,
/// and facets computed over that same matching set.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct LogSearchResult {
    pub hits: Vec<LogSearchHit>,
    pub total_hits: u64,
    pub facets: LogFacets,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_inside_the_cap_is_accepted() {
        assert_eq!(LogWindow::minutes(15).expect("inside").as_minutes(), 15);
        assert_eq!(
            LogWindow::minutes(MAX_WINDOW_MINUTES)
                .expect("the cap itself")
                .as_minutes(),
            MAX_WINDOW_MINUTES
        );
    }

    /// The cap is the server's, so it is refused here and not merely absent
    /// from a dropdown.
    #[test]
    fn a_window_past_the_cap_is_refused_and_names_it() {
        let error = LogWindow::minutes(MAX_WINDOW_MINUTES + 1).expect_err("past the cap");

        assert!(matches!(error, CoreError::InvalidLogWindow { .. }));
        assert!(error.to_string().contains(&MAX_WINDOW_MINUTES.to_string()));
    }

    /// Silently shortening would have the reader take the missing time for a
    /// quiet period, which is the one thing logs must never be made to say.
    #[test]
    fn a_window_past_the_cap_is_not_shortened() {
        assert!(LogWindow::minutes(60 * 24).is_err());
    }

    #[test]
    fn a_window_of_nothing_is_refused() {
        assert!(LogWindow::minutes(0).is_err());
        assert!(LogWindow::minutes(-5).is_err());
    }

    #[test]
    fn every_level_name_parses_case_insensitively() {
        for (text, expected) in [
            ("trace", LogLevel::Trace),
            ("DEBUG", LogLevel::Debug),
            ("Info", LogLevel::Info),
            ("warn", LogLevel::Warn),
            ("ERROR", LogLevel::Error),
            ("fatal", LogLevel::Fatal),
        ] {
            assert_eq!(text.parse::<LogLevel>(), Ok(expected));
        }
    }

    /// `unknown` names a fact about a line, not a floor a caller can ask
    /// for -- asking for it as a floor would be asking to exclude every
    /// classified level, which is never what "read at this severity and
    /// above" means.
    #[test]
    fn unknown_is_not_a_parseable_floor() {
        let error = "unknown".parse::<LogLevel>().expect_err("not a floor");
        assert!(error.contains("unknown"));
    }

    /// The ordering #292 decided on, checked directly rather than trusted to
    /// the order the variants happen to be declared in.
    #[test]
    fn levels_are_ordered_trace_through_fatal() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Fatal);
    }

    /// `unknown` is not on the scale, so it must be added rather than found
    /// by the comparison above -- and it must be there whatever the floor is,
    /// including the strictest one.
    #[test]
    fn every_floor_still_includes_unknown() {
        for floor in LogLevel::ORDERED {
            assert!(floor.and_above().contains(&UNKNOWN_LEVEL), "{floor:?}");
        }
    }

    #[test]
    fn a_floor_of_warn_keeps_warn_error_fatal_and_unknown_only() {
        let levels = LogLevel::Warn.and_above();

        assert_eq!(levels.len(), 4);
        for expected in ["warn", "error", "fatal", "unknown"] {
            assert!(levels.contains(&expected), "missing {expected}: {levels:?}");
        }
        for excluded in ["trace", "debug", "info"] {
            assert!(!levels.contains(&excluded), "should not include {excluded}");
        }
    }

    #[test]
    fn a_floor_of_trace_keeps_everything() {
        assert_eq!(LogLevel::Trace.and_above().len(), 7);
    }

    fn instant(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    #[test]
    fn a_search_window_where_to_is_after_from_is_accepted() {
        let window = LogSearchWindow::new(
            instant("2026-09-01T00:00:00Z"),
            instant("2026-09-02T00:00:00Z"),
        )
        .expect("an ordinary day-long window");

        assert_eq!(window.from, instant("2026-09-01T00:00:00Z"));
        assert_eq!(window.to, instant("2026-09-02T00:00:00Z"));
    }

    /// A relative window would be non-deterministic across two calls; an
    /// absolute one still has to be refused if it is nonsensical.
    #[test]
    fn a_window_ending_at_or_before_it_starts_is_refused() {
        let same_instant = instant("2026-09-01T00:00:00Z");
        assert!(matches!(
            LogSearchWindow::new(same_instant, same_instant),
            Err(CoreError::InvalidLogSearchWindow { .. })
        ));

        assert!(matches!(
            LogSearchWindow::new(
                instant("2026-09-02T00:00:00Z"),
                instant("2026-09-01T00:00:00Z")
            ),
            Err(CoreError::InvalidLogSearchWindow { .. })
        ));
    }

    /// The cap matches the index's own retention: asking further back can
    /// only ever be answered with nothing.
    #[test]
    fn a_window_longer_than_retention_is_refused() {
        let error = LogSearchWindow::new(
            instant("2026-01-01T00:00:00Z"),
            instant("2026-03-01T00:00:00Z"),
        )
        .expect_err("past the retention cap");

        assert!(matches!(error, CoreError::InvalidLogSearchWindow { .. }));
        assert!(error.to_string().contains("30"));
    }

    #[test]
    fn a_window_of_exactly_the_retention_cap_is_accepted() {
        let from = instant("2026-01-01T00:00:00Z");
        let to = from + chrono::Duration::days(LogSearchWindow::MAX_SPAN_DAYS);

        assert!(LogSearchWindow::new(from, to).is_ok());
    }
}
