use std::future::Future;

use crate::domain::action::{ActionBatch, ActionCursor, NormalizedAction};
use crate::domain::HeraldError;

#[cfg_attr(test, mockall::automock)]
pub trait ControlPlaneActionSource: Send + Sync {
    fn fetch_actions(
        &self,
        cursor: Option<ActionCursor>,
    ) -> impl Future<Output = Result<ActionBatch, HeraldError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait MessageBusPublisher: Send + Sync {
    fn publish(
        &self,
        action: NormalizedAction,
    ) -> impl Future<Output = Result<(), HeraldError>> + Send;
}
