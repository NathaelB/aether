use crate::{
    audit::{AuditAction, AuditActor, AuditChange, AuditCursor, AuditTarget},
    organisation::OrganisationId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordAuditEntryCommand {
    pub organisation_id: OrganisationId,
    pub actor: AuditActor,
    pub action: AuditAction,
    pub target: AuditTarget,
    pub change: Option<AuditChange>,
}

impl RecordAuditEntryCommand {
    pub fn new(
        organisation_id: OrganisationId,
        actor: AuditActor,
        action: AuditAction,
        target: AuditTarget,
    ) -> Self {
        Self {
            organisation_id,
            actor,
            action,
            target,
            change: None,
        }
    }

    pub fn with_change(mut self, change: AuditChange) -> Self {
        self.change = Some(change);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListAuditEntriesCommand {
    pub organisation_id: OrganisationId,
    pub cursor: Option<AuditCursor>,
    pub limit: usize,
}

impl ListAuditEntriesCommand {
    /// A page large enough to export the whole trail in a handful of
    /// requests. Chosen over a dedicated export endpoint: a second endpoint
    /// would carry its own copy of the permission check and the cursor
    /// format, and the two would eventually drift.
    pub const MAX_LIMIT: usize = 1000;

    pub fn new(organisation_id: OrganisationId, limit: usize) -> Self {
        Self {
            organisation_id,
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
    use uuid::Uuid;

    #[test]
    fn record_command_starts_without_a_change() {
        let command = RecordAuditEntryCommand::new(
            OrganisationId(Uuid::new_v4()),
            AuditActor::System,
            AuditAction("organisation.plan.changed".to_string()),
            crate::audit::AuditTarget {
                kind: crate::audit::AuditTargetKind::Organisation,
                id: Uuid::new_v4(),
            },
        );

        assert!(command.change.is_none());
    }

    #[test]
    fn a_limit_above_the_maximum_is_clamped() {
        let command = ListAuditEntriesCommand::new(OrganisationId(Uuid::new_v4()), 1_000_000);

        assert_eq!(command.limit, ListAuditEntriesCommand::MAX_LIMIT);
    }

    #[test]
    fn a_limit_of_zero_is_raised_to_one() {
        let command = ListAuditEntriesCommand::new(OrganisationId(Uuid::new_v4()), 0);

        assert_eq!(command.limit, 1);
    }

    #[test]
    fn list_command_sets_cursor() {
        let organisation_id = OrganisationId(Uuid::new_v4());
        let command = ListAuditEntriesCommand::new(organisation_id, 50)
            .with_cursor(AuditCursor::new("cursor-1"));

        assert_eq!(command.organisation_id, organisation_id);
        assert_eq!(command.cursor, Some(AuditCursor::new("cursor-1")));
    }
}
