//! A trail the services' own tests can read back.
//!
//! Shared rather than written per module, for the reason
//! [`crate::platform::fixtures`] is: the services that record fleet actions
//! live in two bounded contexts, and two copies of "collect what was appended"
//! would be two places to change when the port grows a method -- with the one
//! nobody updated being the one whose tests keep passing while the service
//! stops recording.
//!
//! It never fails, which is the point. Most of these tests are about an act
//! rather than about its entry, and a recorder that had to be primed per test
//! would turn every one of them into a test about the trail.

use std::sync::{Arc, Mutex};

use crate::{
    CoreError,
    audit::{
        AuditCursor,
        fleet::{FleetAuditBatch, FleetAuditEntry, ports::FleetAuditRepository},
    },
};

#[derive(Clone, Default)]
pub struct Recording(Arc<Mutex<Vec<FleetAuditEntry>>>);

impl Recording {
    pub fn new() -> Self {
        Self::default()
    }

    /// What was appended, in the order it was appended.
    pub fn entries(&self) -> Vec<FleetAuditEntry> {
        self.0.lock().expect("a test holds this alone").clone()
    }

    /// The single entry a test expects, or a failure naming how many there
    /// were instead.
    ///
    /// An assertion on `entries()[0]` would pass just as happily on a service
    /// that recorded the same act twice, and recording twice is the failure
    /// this shape exists to catch.
    pub fn only(&self) -> FleetAuditEntry {
        let entries = self.entries();
        assert_eq!(entries.len(), 1, "expected exactly one entry");

        entries.into_iter().next().expect("a single entry")
    }
}

impl FleetAuditRepository for Recording {
    async fn append(&self, entry: FleetAuditEntry) -> Result<(), CoreError> {
        self.0.lock().expect("a test holds this alone").push(entry);
        Ok(())
    }

    async fn list(
        &self,
        _cursor: Option<AuditCursor>,
        limit: usize,
    ) -> Result<FleetAuditBatch, CoreError> {
        let mut entries = self.entries();
        entries.reverse();
        entries.truncate(limit);

        Ok(FleetAuditBatch {
            entries,
            next_cursor: None,
        })
    }
}
