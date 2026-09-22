//! The control plane's own Quickwit search adapters -- one per index family
//! (`logs`, `traces`), split out once `logs.rs` alone passed 900 lines.
//! Both speak to the same Quickwit deployment over the same HTTP API and
//! share nothing beyond that; see each module's own doc comment.

pub mod logs;
pub mod traces;

pub use logs::QuickwitLogSearchIndex;
pub use traces::QuickwitTraceSearchIndex;
