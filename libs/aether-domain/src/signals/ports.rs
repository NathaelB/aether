use std::future::Future;

use chrono::{DateTime, Utc};

use crate::CoreError;

use super::{Signal, SignalKind, SignalSubject};

#[cfg_attr(test, mockall::automock)]
pub trait SignalRepository: Send + Sync {
    fn write(&self, signal: Signal) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn close(
        &self,
        dedup_key: &str,
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn list_open(
        &self,
        kind_filter: Option<SignalKind>,
        subject_filter: Option<SignalSubject>,
        limit: usize,
        cursor: Option<String>,
    ) -> impl Future<Output = Result<SignalListPage, CoreError>> + Send;
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignalListPage {
    pub signals: Vec<Signal>,
    pub next_cursor: Option<String>,
}
