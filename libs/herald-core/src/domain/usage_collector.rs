//! Turns cumulative counters read from IAM instances into one-minute buckets.
//!
//! Everything here is pure: it is handed readings and a clock, and it holds
//! the only state Herald keeps between cycles. The reason it is pure is that
//! the two properties this feature is judged on -- a restart that neither gaps
//! nor duplicates, and an unreachable instance that leaves a hole rather than
//! a zero -- are properties of this arithmetic, and they are only cheap to
//! falsify while no socket is involved.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};

use crate::domain::entities::deployment::DeploymentId;
use crate::domain::entities::usage::{
    BUCKET_SECONDS, CounterSample, UsageBucket, UsageMetric, UsagePoint,
};

/// How long a bucket keeps being reported after it closes.
///
/// Every cycle sends the whole window, not only what is new. The control
/// plane's write is idempotent on `(deployment, metric, bucket)`, so a repeat
/// costs one row update and buys the only recovery Herald has: a control
/// plane that was unreachable for less than this window catches up on its own,
/// with nothing missing and nothing counted twice.
pub const RETENTION_MINUTES: i64 = 15;

#[derive(Default)]
struct DeploymentUsage {
    /// The last reading, kept as the baseline the next delta is taken against.
    last: Option<CounterSample>,

    /// The first bucket Herald has watched from its beginning.
    ///
    /// Herald almost never starts on a minute boundary, so the minute it comes
    /// up in is one it joined halfway through. Reporting that minute would
    /// overwrite whatever the previous process had already written for it with
    /// a smaller number, which is a worse answer than the one already there.
    reportable_from: Option<UsageBucket>,

    buckets: BTreeMap<UsageBucket, BTreeMap<UsageMetric, u64>>,
}

pub struct UsageCollector {
    retention: Duration,
    deployments: HashMap<DeploymentId, DeploymentUsage>,
}

impl Default for UsageCollector {
    fn default() -> Self {
        Self::new(Duration::minutes(RETENTION_MINUTES))
    }
}

impl UsageCollector {
    pub fn new(retention: Duration) -> Self {
        Self {
            retention,
            deployments: HashMap::new(),
        }
    }

    /// Folds one successful reading into the deployment's buckets.
    ///
    /// Called only when an instance actually answered. There is deliberately
    /// no counterpart for a failed read: the way Herald says "this minute was
    /// not observed" is by leaving nothing behind, and a method that recorded
    /// the failure would be one refactor away from recording it as a zero.
    pub fn observe(&mut self, deployment_id: &DeploymentId, sample: CounterSample) {
        let entry = self.deployments.entry(deployment_id.clone()).or_default();
        let bucket = UsageBucket::containing(sample.taken_at);
        let previous = entry.last.replace(sample.clone());

        let Some(previous) =
            previous.filter(|before| attributable(before.taken_at, sample.taken_at))
        else {
            // No usable baseline. Either this is the first reading, or the
            // previous one is old enough that the difference spans more than
            // one bucket -- and a difference that covers four minutes credited
            // to one of them is a minute reporting usage that did not happen
            // in it. Take this reading as the new baseline instead and resume
            // at the next whole minute.
            entry.reportable_from = Some(bucket.next());
            return;
        };

        if entry
            .reportable_from
            .is_none_or(|reportable_from| bucket < reportable_from)
        {
            return;
        }

        let slot = entry.buckets.entry(bucket).or_default();

        for (metric, total) in &sample.totals {
            let Some(before) = previous.totals.get(metric) else {
                // First sighting of this metric: there is no baseline to
                // subtract, and the counter's own total is not this minute's.
                continue;
            };

            let delta = if total >= before {
                total - before
            } else {
                // The instance restarted and its counters went back to zero,
                // so what is there now is what it has counted since.
                *total
            };

            // Entered even when the delta is zero: the instance answered, and
            // "answered, nobody did anything" is a fact worth reporting. It is
            // the minute nobody answered for that must stay absent.
            *slot.entry(*metric).or_insert(0) += delta;
        }
    }

    /// Everything currently worth reporting for a deployment, oldest first.
    ///
    /// Includes the minute still in progress. Its value is partial and will be
    /// sent again, larger, on the next cycle -- which is safe precisely
    /// because the control plane overwrites rather than adds, and which means
    /// a Herald that dies mid-minute has already reported most of that minute.
    pub fn points(&self, deployment_id: &DeploymentId) -> Vec<UsagePoint> {
        let Some(usage) = self.deployments.get(deployment_id) else {
            return Vec::new();
        };

        usage
            .buckets
            .iter()
            .flat_map(|(bucket, metrics)| {
                metrics.iter().map(move |(metric, value)| UsagePoint {
                    metric: *metric,
                    bucket: *bucket,
                    value: *value,
                })
            })
            .collect()
    }

    /// Drops buckets that have aged out of the retention window.
    pub fn evict(&mut self, now: DateTime<Utc>) {
        let oldest = UsageBucket::containing(now - self.retention);

        for usage in self.deployments.values_mut() {
            usage.buckets.retain(|bucket, _| *bucket >= oldest);
        }
    }

    /// Forgets every deployment not in `keep`.
    ///
    /// Without it a data plane that has churned through deployments carries
    /// their buckets, and their baselines, for as long as the process lives.
    pub fn retain(&mut self, keep: &HashSet<DeploymentId>) {
        self.deployments.retain(|id, _| keep.contains(id));
    }
}

/// Whether the difference between two readings belongs to a single minute.
///
/// One bucket wide is the limit by construction: a longer interval necessarily
/// covers a minute Herald has no separate reading for, and there is no honest
/// way to split it.
fn attributable(before: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    let elapsed = now - before;

    elapsed > Duration::zero() && elapsed <= Duration::seconds(BUCKET_SECONDS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    fn deployment() -> DeploymentId {
        DeploymentId::new("dep-1")
    }

    fn sample(taken_at: &str, requests: u64) -> CounterSample {
        CounterSample::new(at(taken_at)).with(UsageMetric::Requests, requests)
    }

    fn requests_in(collector: &UsageCollector, bucket_start: &str) -> Option<u64> {
        collector
            .points(&deployment())
            .into_iter()
            .find(|point| {
                point.metric == UsageMetric::Requests && point.bucket.start() == at(bucket_start)
            })
            .map(|point| point.value)
    }

    /// The first reading only says where the counters stand. A per-minute
    /// number needs two of them.
    #[test]
    fn a_single_reading_reports_nothing() {
        let mut collector = UsageCollector::default();

        collector.observe(&deployment(), sample("2026-01-01T10:00:00Z", 500));

        assert!(collector.points(&deployment()).is_empty());
    }

    #[test]
    fn two_readings_report_the_difference_between_them() {
        let mut collector = UsageCollector::default();

        collector.observe(&deployment(), sample("2026-01-01T10:00:20Z", 500));
        collector.observe(&deployment(), sample("2026-01-01T10:01:10Z", 530));

        assert_eq!(requests_in(&collector, "2026-01-01T10:01:00Z"), Some(30));
    }

    /// Several readings inside one minute add up into that minute rather than
    /// each opening one of their own -- this is the pre-aggregation the
    /// control plane is being spared.
    #[test]
    fn readings_within_a_minute_accumulate_into_one_bucket() {
        let mut collector = UsageCollector::default();

        collector.observe(&deployment(), sample("2026-01-01T10:00:00Z", 500));
        collector.observe(&deployment(), sample("2026-01-01T10:01:00Z", 500));
        collector.observe(&deployment(), sample("2026-01-01T10:01:15Z", 510));
        collector.observe(&deployment(), sample("2026-01-01T10:01:30Z", 511));
        collector.observe(&deployment(), sample("2026-01-01T10:01:45Z", 520));

        assert_eq!(requests_in(&collector, "2026-01-01T10:01:00Z"), Some(20));
    }

    /// The whole point of the design. An instance that answered and had no
    /// traffic reports a zero; an instance that did not answer reports
    /// nothing, and the two must not end up looking alike.
    #[test]
    fn an_instance_that_answered_with_no_traffic_reports_a_zero() {
        let mut collector = UsageCollector::default();

        collector.observe(&deployment(), sample("2026-01-01T10:00:20Z", 500));
        collector.observe(&deployment(), sample("2026-01-01T10:01:10Z", 500));

        assert_eq!(
            requests_in(&collector, "2026-01-01T10:01:00Z"),
            Some(0),
            "a deployment nobody used is a real zero"
        );
    }

    /// The other half of the same fact: nothing was observed for 10:01, so
    /// nothing is reported for it. A zero here would tell the read side that
    /// the deployment was idle, when what actually happened is that Herald
    /// could not see it.
    #[test]
    fn a_minute_with_no_reading_is_missing_rather_than_zero() {
        let mut collector = UsageCollector::default();

        collector.observe(&deployment(), sample("2026-01-01T10:00:00Z", 500));
        collector.observe(&deployment(), sample("2026-01-01T10:00:30Z", 500));
        // 10:01 and 10:02 pass with the instance unreachable, so nothing is
        // observed for them at all.
        collector.observe(&deployment(), sample("2026-01-01T10:03:10Z", 900));
        collector.observe(&deployment(), sample("2026-01-01T10:04:10Z", 910));

        assert_eq!(
            requests_in(&collector, "2026-01-01T10:01:00Z"),
            None,
            "an unreachable minute must produce no bucket at all"
        );
        assert_eq!(requests_in(&collector, "2026-01-01T10:02:00Z"), None);
    }

    /// The 400 requests that arrived while the instance was unreachable are
    /// not credited to the minute Herald got back in. Losing them is the
    /// honest outcome; parking them all in one minute would report traffic in
    /// a minute it did not happen in.
    #[test]
    fn usage_from_an_unobserved_stretch_is_not_dumped_into_the_minute_it_was_noticed() {
        let mut collector = UsageCollector::default();

        collector.observe(&deployment(), sample("2026-01-01T10:00:00Z", 500));
        collector.observe(&deployment(), sample("2026-01-01T10:03:10Z", 900));

        assert_eq!(requests_in(&collector, "2026-01-01T10:03:00Z"), None);
    }

    /// An IAM pod that restarts takes its counters back to zero. Subtracting
    /// the old total would underflow, and clamping it to zero would throw away
    /// everything the new process has served.
    #[test]
    fn a_counter_that_restarted_is_read_as_the_total_since_it_restarted() {
        let mut collector = UsageCollector::default();

        collector.observe(&deployment(), sample("2026-01-01T10:00:20Z", 5_000));
        collector.observe(&deployment(), sample("2026-01-01T10:01:10Z", 7));

        assert_eq!(requests_in(&collector, "2026-01-01T10:01:00Z"), Some(7));
    }

    /// Herald comes up at 10:00:20, so it saw two thirds of 10:00. Reporting
    /// that as the whole minute would replace a fuller value the process it is
    /// replacing may already have written.
    #[test]
    fn the_minute_herald_started_halfway_through_is_not_reported() {
        let mut collector = UsageCollector::default();

        collector.observe(&deployment(), sample("2026-01-01T10:00:20Z", 500));
        collector.observe(&deployment(), sample("2026-01-01T10:00:35Z", 510));
        collector.observe(&deployment(), sample("2026-01-01T10:00:50Z", 520));

        assert_eq!(
            requests_in(&collector, "2026-01-01T10:00:00Z"),
            None,
            "a partly watched minute is not a minute"
        );

        collector.observe(&deployment(), sample("2026-01-01T10:01:05Z", 530));
        assert_eq!(
            requests_in(&collector, "2026-01-01T10:01:00Z"),
            Some(10),
            "the first whole minute after starting is reported normally"
        );
    }

    /// A bucket is offered again on every cycle for as long as it is retained.
    /// That is what makes a lost report harmless: nothing has to be
    /// remembered about whether a write landed, because the next cycle sends
    /// it again and the write overwrites.
    #[test]
    fn a_closed_bucket_keeps_being_offered_with_the_same_value() {
        let mut collector = UsageCollector::default();

        collector.observe(&deployment(), sample("2026-01-01T10:00:20Z", 500));
        collector.observe(&deployment(), sample("2026-01-01T10:01:10Z", 530));

        let first = collector.points(&deployment());

        collector.observe(&deployment(), sample("2026-01-01T10:02:00Z", 530));

        let second = collector.points(&deployment());

        assert!(
            second.contains(&first[0]),
            "{second:?} must still carry {first:?}"
        );
    }

    #[test]
    fn buckets_older_than_the_retention_window_are_dropped() {
        let mut collector = UsageCollector::new(Duration::minutes(2));

        collector.observe(&deployment(), sample("2026-01-01T10:00:00Z", 500));
        collector.observe(&deployment(), sample("2026-01-01T10:01:00Z", 530));
        collector.observe(&deployment(), sample("2026-01-01T10:02:00Z", 560));

        collector.evict(at("2026-01-01T10:04:30Z"));

        assert_eq!(requests_in(&collector, "2026-01-01T10:01:00Z"), None);
        assert_eq!(requests_in(&collector, "2026-01-01T10:02:00Z"), Some(30));
    }

    #[test]
    fn a_deployment_no_longer_owned_is_forgotten() {
        let mut collector = UsageCollector::default();

        collector.observe(&deployment(), sample("2026-01-01T10:00:00Z", 500));
        collector.observe(&deployment(), sample("2026-01-01T10:01:00Z", 530));

        collector.retain(&HashSet::from([DeploymentId::new("dep-2")]));

        assert!(collector.points(&deployment()).is_empty());
    }

    /// A metric the instance only started exposing partway through has no
    /// baseline, and its running total is not one minute's worth of it.
    #[test]
    fn a_metric_that_appears_for_the_first_time_contributes_no_bucket() {
        let mut collector = UsageCollector::default();

        collector.observe(&deployment(), sample("2026-01-01T10:00:00Z", 500));
        collector.observe(
            &deployment(),
            sample("2026-01-01T10:01:00Z", 500).with(UsageMetric::TokenEvents, 90),
        );

        let token_events: Vec<_> = collector
            .points(&deployment())
            .into_iter()
            .filter(|point| point.metric == UsageMetric::TokenEvents)
            .collect();

        assert!(token_events.is_empty(), "got {token_events:?}");
    }

    /// A restart, end to end. The process that died had been reporting; the
    /// one that replaces it must not credit the traffic that accumulated while
    /// nothing was watching to the minute it happened to come up in.
    #[test]
    fn a_restarted_herald_resumes_without_inventing_a_busy_minute() {
        let mut before = UsageCollector::default();
        before.observe(&deployment(), sample("2026-01-01T10:00:00Z", 500));
        before.observe(&deployment(), sample("2026-01-01T10:01:00Z", 530));

        assert_eq!(requests_in(&before, "2026-01-01T10:01:00Z"), Some(30));

        // Herald dies at 10:01:30 and comes back at 10:02:40, by which time
        // the instance has served another 400 requests.
        let mut after = UsageCollector::default();
        after.observe(&deployment(), sample("2026-01-01T10:02:40Z", 930));
        after.observe(&deployment(), sample("2026-01-01T10:02:55Z", 935));
        after.observe(&deployment(), sample("2026-01-01T10:03:10Z", 940));

        assert_eq!(
            requests_in(&after, "2026-01-01T10:02:00Z"),
            None,
            "the minute it restarted in was only partly watched"
        );
        assert_eq!(
            requests_in(&after, "2026-01-01T10:03:00Z"),
            Some(5),
            "the first whole minute after the restart is reported, and reports only its own traffic"
        );
    }
}
