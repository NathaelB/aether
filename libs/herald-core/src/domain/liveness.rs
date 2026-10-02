use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

const MIN_MAX_AGE: Duration = Duration::from_secs(60);
const MAX_AGE_IN_POLLS: u32 = 3;

pub trait Clock: Send + Sync {
    fn elapsed(&self) -> Duration;
}

pub struct SystemClock {
    started: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

struct Inner<C> {
    clock: C,
    max_age: Duration,
    last_cycle_millis: AtomicU64,
    cycled: AtomicBool,
}

/// Whether Herald's own sync loop is turning, and nothing else.
///
/// It must never look at the control plane, the broker, the cluster or the
/// search index: a Herald that cannot reach them is degraded, not dead, and a
/// restart would not help and would drop the traces it is receiving.
///
/// Before the first cycle the age is counted from construction, so a Herald
/// that never gets its loop going is still reported dead once the bound passes.
pub struct Liveness<C> {
    inner: Arc<Inner<C>>,
}

impl<C> Clone for Liveness<C> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<C: Clock> Liveness<C> {
    pub fn new(clock: C, poll_interval: Duration) -> Self {
        let max_age = (poll_interval * MAX_AGE_IN_POLLS).max(MIN_MAX_AGE);
        let started = clock.elapsed().as_millis() as u64;
        Self {
            inner: Arc::new(Inner {
                clock,
                max_age,
                last_cycle_millis: AtomicU64::new(started),
                cycled: AtomicBool::new(false),
            }),
        }
    }

    pub fn max_age(&self) -> Duration {
        self.inner.max_age
    }

    pub fn record_cycle(&self) {
        let now = self.inner.clock.elapsed().as_millis() as u64;
        self.inner.last_cycle_millis.store(now, Ordering::Release);
        self.inner.cycled.store(true, Ordering::Release);
    }

    pub fn age(&self) -> Duration {
        let now = self.inner.clock.elapsed().as_millis() as u64;
        let last = self.inner.last_cycle_millis.load(Ordering::Acquire);
        Duration::from_millis(now.saturating_sub(last))
    }

    pub fn is_alive(&self) -> bool {
        self.age() < self.inner.max_age
    }

    pub fn is_ready(&self) -> bool {
        self.inner.cycled.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Default)]
    pub struct FakeClock(Arc<AtomicU64>);

    impl FakeClock {
        fn advance(&self, by: Duration) {
            self.0.fetch_add(by.as_millis() as u64, Ordering::SeqCst);
        }
    }

    impl Clock for FakeClock {
        fn elapsed(&self) -> Duration {
            Duration::from_millis(self.0.load(Ordering::SeqCst))
        }
    }

    #[test]
    fn a_fresh_cycle_is_alive_and_ready() {
        let clock = FakeClock::default();
        let liveness = Liveness::new(clock.clone(), Duration::from_secs(15));

        liveness.record_cycle();
        clock.advance(Duration::from_secs(10));

        assert!(liveness.is_alive());
        assert!(liveness.is_ready());
    }

    #[test]
    fn a_stale_cycle_is_dead_but_stays_ready() {
        let clock = FakeClock::default();
        let liveness = Liveness::new(clock.clone(), Duration::from_secs(15));

        liveness.record_cycle();
        clock.advance(Duration::from_secs(60));

        assert!(!liveness.is_alive());
        assert!(liveness.is_ready());
    }

    #[test]
    fn a_cycle_just_inside_the_bound_is_alive() {
        let clock = FakeClock::default();
        let liveness = Liveness::new(clock.clone(), Duration::from_secs(15));

        liveness.record_cycle();
        clock.advance(Duration::from_millis(59_999));

        assert!(liveness.is_alive());
    }

    #[test]
    fn never_recorded_is_not_ready_and_alive_only_within_the_bound() {
        let clock = FakeClock::default();
        clock.advance(Duration::from_secs(1000));
        let liveness = Liveness::new(clock.clone(), Duration::from_secs(15));

        clock.advance(Duration::from_secs(59));
        assert!(liveness.is_alive());
        assert!(!liveness.is_ready());

        clock.advance(Duration::from_secs(1));
        assert!(!liveness.is_alive());
        assert!(!liveness.is_ready());
    }

    #[test]
    fn recording_again_revives_a_stale_loop() {
        let clock = FakeClock::default();
        let liveness = Liveness::new(clock.clone(), Duration::from_secs(15));
        clock.advance(Duration::from_secs(120));
        assert!(!liveness.is_alive());

        liveness.record_cycle();

        assert!(liveness.is_alive());
    }

    #[test]
    fn the_bound_is_three_polls_with_a_floor_of_sixty_seconds() {
        let clock = FakeClock::default();

        assert_eq!(
            Liveness::new(clock.clone(), Duration::from_secs(3)).max_age(),
            Duration::from_secs(60)
        );
        assert_eq!(
            Liveness::new(clock.clone(), Duration::from_secs(15)).max_age(),
            Duration::from_secs(60)
        );
        assert_eq!(
            Liveness::new(clock, Duration::from_secs(60)).max_age(),
            Duration::from_secs(180)
        );
    }

    #[test]
    fn clones_share_the_same_state() {
        let clock = FakeClock::default();
        let liveness = Liveness::new(clock, Duration::from_secs(15));
        let other = liveness.clone();

        other.record_cycle();

        assert!(liveness.is_ready());
    }
}
