//! Turning a relayed line into the document the search index expects.
//!
//! The six fields below are the mapping #293 froze
//! (`docker/quickwit/index-config.template.yaml`): this is the one place
//! that maps [`LogLine`] and [`LogStreamRequest`] onto it, so the mapping
//! only has to be reasoned about once.

use chrono::{DateTime, Utc};
use serde::Serialize;

use super::entities::deployment::DeploymentId;
use super::entities::logs::{LogLine, LogStreamRequest, OrganisationId};

/// The level Herald could not find in a line's own text.
const UNKNOWN_LEVEL: &str = "unknown";

/// One line, shaped for the index rather than for the live tail.
///
/// Deliberately not [`LogLine`] with fields bolted on: the console's
/// `LogLine` is what the live relay sends today and this chantier does not
/// touch it (#292's V1 ordering entry), while this type exists only on the
/// path to the index and carries the one field a pod's raw text never has --
/// `level`, derived by [`derive_level`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogIndexDocument {
    pub timestamp: DateTime<Utc>,
    pub organisation_id: OrganisationId,
    pub deployment_id: DeploymentId,
    pub source: String,
    pub level: String,
    pub message: String,
}

impl LogIndexDocument {
    /// Builds the document one relayed line ships as.
    pub fn from_line(request: &LogStreamRequest, line: &LogLine) -> Self {
        let message = strip_ansi(&line.message);

        Self {
            timestamp: line.at,
            organisation_id: request.organisation_id.clone(),
            deployment_id: request.deployment_id.clone(),
            source: line.source.clone(),
            level: derive_level(&message),
            message,
        }
    }
}

/// Drops the terminal colouring a container writes into its own output.
///
/// Every Rust service here logs through `tracing_subscriber`, which colours
/// by default, and Kubernetes hands the bytes over untouched. Left in, the
/// escape sequence defeats [`derive_level`] outright -- `\x1b[0m` ends in an
/// alphabetic `m`, so trimming non-letters off `INFO\x1b[0m` stops there and
/// the token never matches a level name. Every line from a coloured service
/// would be `unknown`, which makes the level filter and its facet useless.
/// The raw sequence would also reach the search screen as literal text.
fn strip_ansi(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut chars = message.chars();

    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        // CSI: `\x1b[` then parameter bytes, ended by a byte in `@`..=`~`.
        // Anything else after the escape is a two-character sequence, whose
        // second character `chars.next()` has already consumed.
        if let Some('[') = chars.next() {
            for c in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&c) {
                    break;
                }
            }
        }
    }

    out
}

/// Guesses a line's level from its own text.
///
/// Kubernetes gives raw text; neither Keycloak nor Ferriskey attach a level
/// Herald can read structurally. Both write it as a bare, upper-case word
/// near the start of the line -- Ferriskey through `tracing_subscriber`'s
/// default formatter (e.g. `2026-01-01T10:00:00.123456Z  INFO
/// ferriskey_api::handler: started in 4.2s`), Keycloak through its Quarkus
/// logging pattern (e.g. `2026-01-01 10:00:00,123 WARN  [org.keycloak.events]
/// (executor-thread-1) type=LOGIN_ERROR ...`). The heuristic looks for
/// exactly that: the first whitespace-delimited word that, once surrounding
/// punctuation is trimmed, is one of the known level names.
///
/// Deliberately narrow. A JSON payload, a stack-trace continuation line, or a
/// product that changes its format falls back to [`UNKNOWN_LEVEL`] rather
/// than guessing further -- an unindexed level is a gap V2 can filter around,
/// a wrong one is a fact the index would assert that is not true.
pub fn derive_level(message: &str) -> String {
    message
        .split_whitespace()
        .find_map(|word| {
            let trimmed = word.trim_matches(|c: char| !c.is_ascii_alphabetic());
            match trimmed.to_ascii_uppercase().as_str() {
                "TRACE" => Some("trace"),
                "DEBUG" => Some("debug"),
                "INFO" => Some("info"),
                "WARN" | "WARNING" => Some("warn"),
                "ERROR" => Some("error"),
                "FATAL" => Some("fatal"),
                _ => None,
            }
        })
        .unwrap_or(UNKNOWN_LEVEL)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::dataplane::DataPlaneId;
    use crate::domain::entities::deployment::DeploymentKind;
    use crate::domain::entities::logs::LogSessionId;
    use uuid::Uuid;

    #[test]
    fn a_ferriskey_style_line_is_read_as_info() {
        assert_eq!(
            derive_level(
                "2026-01-01T10:00:00.123456Z  INFO ferriskey_api::handler: started in 4.2s"
            ),
            "info"
        );
    }

    #[test]
    fn a_keycloak_style_line_is_read_as_warn() {
        assert_eq!(
            derive_level(
                "2026-01-01 10:00:00,123 WARN  [org.keycloak.events] (executor-thread-1) type=LOGIN_ERROR"
            ),
            "warn"
        );
    }

    #[test]
    fn warning_spelled_out_is_read_as_warn() {
        assert_eq!(derive_level("a WARNING was raised"), "warn");
    }

    #[test]
    fn error_and_debug_and_trace_and_fatal_are_all_read() {
        assert_eq!(derive_level("ERROR something broke"), "error");
        assert_eq!(derive_level("DEBUG the request body"), "debug");
        assert_eq!(derive_level("TRACE entering handler"), "trace");
        assert_eq!(derive_level("FATAL cannot continue"), "fatal");
    }

    /// A word that merely contains a level name is not a match: reading
    /// "information" as `info` would be a level the line never claimed.
    #[test]
    fn a_word_that_only_contains_a_level_name_does_not_match() {
        assert_eq!(
            derive_level("for your information, nothing failed"),
            "unknown"
        );
    }

    #[test]
    fn a_line_naming_no_level_falls_back_to_unknown() {
        assert_eq!(
            derive_level("{\"msg\":\"structured, no bare level\"}"),
            "unknown"
        );
    }

    #[test]
    fn a_level_wrapped_in_brackets_or_commas_still_matches() {
        assert_eq!(derive_level("[ERROR] something broke"), "error");
        assert_eq!(derive_level("INFO, started"), "info");
    }

    fn request() -> LogStreamRequest {
        LogStreamRequest {
            deployment_id: DeploymentId::new("22222222-2222-2222-2222-222222222222"),
            dataplane_id: DataPlaneId::new("11111111-1111-1111-1111-111111111111"),
            organisation_id: OrganisationId::new("55555555-5555-5555-5555-555555555555"),
            namespace: "aether-acme".to_string(),
            kind: DeploymentKind::Ferriskey,
            session_id: LogSessionId(Uuid::nil()),
            since_minutes: 5,
        }
    }

    #[test]
    fn a_document_carries_the_request_tenancy_and_the_lines_own_fields() {
        let line = LogLine {
            at: Utc::now(),
            source: "ferriskey-api".to_string(),
            message: "ERROR the pool is exhausted".to_string(),
        };

        let document = LogIndexDocument::from_line(&request(), &line);

        assert_eq!(document.organisation_id, request().organisation_id);
        assert_eq!(document.deployment_id, request().deployment_id);
        assert_eq!(document.source, "ferriskey-api");
        assert_eq!(document.level, "error");
        assert_eq!(document.message, "ERROR the pool is exhausted");
        assert_eq!(document.timestamp, line.at);
    }

    /// Captured verbatim from a Ferriskey pod on a real cluster. Every test
    /// above feeds hand-written lines, which is exactly why none of them
    /// caught that a coloured service indexes as `unknown`.
    const COLOURED: &str = "\u{1b}[2m2026-09-20T22:18:35.809043Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m \u{1b}[2mferriskey_api\u{1b}[0m\u{1b}[2m:\u{1b}[0m listening on 0.0.0.0:3333";

    #[test]
    fn a_coloured_line_is_read_as_its_real_level() {
        assert_eq!(derive_level(&strip_ansi(COLOURED)), "info");
    }

    #[test]
    fn a_shipped_document_carries_no_escape_sequences() {
        let line = LogLine {
            at: Utc::now(),
            source: "ferriskey-api".to_string(),
            message: COLOURED.to_string(),
        };

        let document = LogIndexDocument::from_line(&request(), &line);

        assert!(
            !document.message.contains('\u{1b}'),
            "{:?}",
            document.message
        );
        assert_eq!(
            document.message,
            "2026-09-20T22:18:35.809043Z  INFO ferriskey_api: listening on 0.0.0.0:3333"
        );
        assert_eq!(document.level, "info");
    }

    #[test]
    fn a_line_with_no_colouring_is_left_alone() {
        assert_eq!(strip_ansi("WARN plain text"), "WARN plain text");
    }
}
