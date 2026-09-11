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

/// An open read. Lives as long as the connection asking for it, and not a
/// moment longer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSession {
    pub id: LogSessionId,
    pub deployment_id: DeploymentId,
    pub window: LogWindow,
    pub opened_at: DateTime<Utc>,
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
}
