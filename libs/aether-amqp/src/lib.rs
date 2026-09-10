//! Connecting to the broker, and waiting for it when it is not there yet.
//!
//! Herald and Genesis start beside RabbitMQ in the same chart. On a fresh
//! install the broker is not accepting connections yet, and both processes
//! used to exit(1) and let Kubernetes restart them. That converges, but it
//! spends the one signal an operator has: three restarts on a fresh install
//! looks exactly like a bad secret or an unreachable control plane.
//!
//! A broker that is not up yet is an expected state during startup. A broker
//! that is still not up after the budget is a real failure, and exiting then
//! is right -- so the wait is bounded rather than infinite.

use std::time::Duration;

use lapin::{Connection, ConnectionProperties};
use tracing::{info, warn};

/// First wait between attempts. Short: most of the time the broker is seconds
/// away, and a long first wait is pure added startup latency.
const BASE_DELAY: Duration = Duration::from_millis(250);

/// Ceiling on the wait between attempts.
const MAX_DELAY: Duration = Duration::from_secs(5);

/// How long to keep trying before giving up.
///
/// Long enough for a broker that is slow to start -- a cold image pull, a busy
/// node -- and short enough that a genuinely unreachable broker still surfaces
/// as a failed pod rather than one that hangs looking healthy.
pub const DEFAULT_BUDGET: Duration = Duration::from_secs(120);

/// Doubling from [`BASE_DELAY`], capped at [`MAX_DELAY`].
fn delay_for(attempt: u32) -> Duration {
    BASE_DELAY
        .saturating_mul(2_u32.saturating_pow(attempt.min(16)))
        .min(MAX_DELAY)
}

/// Connects to the broker, retrying until `budget` is spent.
///
/// Returns the last error if the budget runs out, so the caller reports why it
/// could not connect rather than that it timed out.
pub async fn connect_with_retry(
    amqp_url: &str,
    budget: Duration,
) -> Result<Connection, lapin::Error> {
    let started = tokio::time::Instant::now();
    let mut attempt = 0;

    loop {
        match Connection::connect(amqp_url, ConnectionProperties::default()).await {
            Ok(connection) => {
                if attempt > 0 {
                    info!(attempt, "connected to the broker");
                }
                return Ok(connection);
            }
            Err(err) => {
                let elapsed = started.elapsed();
                let delay = delay_for(attempt);

                if elapsed + delay >= budget {
                    return Err(err);
                }

                warn!(
                    attempt,
                    retry_in_ms = delay.as_millis() as u64,
                    "broker not reachable yet: {err}"
                );

                tokio::time::sleep(delay).await;
                attempt = attempt.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_wait_is_short() {
        assert_eq!(delay_for(0), BASE_DELAY);
    }

    #[test]
    fn waits_double() {
        assert_eq!(delay_for(1), BASE_DELAY * 2);
        assert_eq!(delay_for(2), BASE_DELAY * 4);
    }

    /// Without the cap, doubling reaches minutes within a dozen attempts, and
    /// a broker that came up in the meantime goes unnoticed for most of it.
    #[test]
    fn waits_are_capped() {
        assert_eq!(delay_for(10), MAX_DELAY);
        assert_eq!(delay_for(u32::MAX), MAX_DELAY);
    }

    /// The budget exists so a genuinely unreachable broker still fails. An
    /// unbounded retry would turn a misconfiguration into a pod that looks
    /// healthy and never does any work.
    #[tokio::test(start_paused = true)]
    async fn gives_up_once_the_budget_is_spent() {
        let started = tokio::time::Instant::now();

        let result = connect_with_retry("amqp://127.0.0.1:1/%2f", Duration::from_secs(3)).await;

        assert!(result.is_err(), "nothing is listening on that port");
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "gave up late: {:?}",
            started.elapsed()
        );
    }

    /// A budget smaller than the first wait must still make exactly one
    /// attempt, rather than returning without having tried at all.
    #[tokio::test(start_paused = true)]
    async fn tries_once_even_with_no_budget() {
        let result = connect_with_retry("amqp://127.0.0.1:1/%2f", Duration::ZERO).await;

        assert!(result.is_err());
    }

    /// Real wall-clock proof that the wait actually happens and is bounded:
    /// the paused-clock tests above would pass even if every sleep were a
    /// no-op.
    #[tokio::test]
    #[ignore = "takes ~2s of real time; run with --ignored"]
    async fn waits_then_gives_up_in_real_time() {
        let started = std::time::Instant::now();
        let result = connect_with_retry("amqp://127.0.0.1:1/%2f", Duration::from_secs(2)).await;
        let elapsed = started.elapsed();

        assert!(result.is_err());
        assert!(
            elapsed >= Duration::from_millis(400),
            "did not actually wait: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(3),
            "overshot the budget: {elapsed:?}"
        );
    }
}
