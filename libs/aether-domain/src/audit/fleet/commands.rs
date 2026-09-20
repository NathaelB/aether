use crate::audit::AuditCursor;

/// Reading the installation's trail.
///
/// No organisation, and no way to add one: the shape of this command is half
/// of what keeps a customer out of the fleet trail. The other half is that
/// [`super::ports::FleetAuditRepository::list`] takes nothing to filter on
/// either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListFleetAuditEntriesCommand {
    pub cursor: Option<AuditCursor>,
    pub limit: usize,
}

impl ListFleetAuditEntriesCommand {
    /// The same ceiling as the organisation trail, for the same reason: a
    /// caller exporting the whole trail does it in a handful of requests
    /// rather than through a second endpoint carrying its own copy of the
    /// permission check and the cursor format.
    pub const MAX_LIMIT: usize = 1000;

    pub fn new(limit: usize) -> Self {
        Self {
            cursor: None,
            limit: limit.clamp(1, Self::MAX_LIMIT),
        }
    }

    pub fn with_cursor(mut self, cursor: AuditCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_limit_above_the_maximum_is_clamped() {
        assert_eq!(
            ListFleetAuditEntriesCommand::new(1_000_000).limit,
            ListFleetAuditEntriesCommand::MAX_LIMIT
        );
    }

    #[test]
    fn a_limit_of_zero_is_raised_to_one() {
        assert_eq!(ListFleetAuditEntriesCommand::new(0).limit, 1);
    }

    #[test]
    fn a_command_starts_at_the_newest_entry() {
        assert!(ListFleetAuditEntriesCommand::new(50).cursor.is_none());
    }
}
