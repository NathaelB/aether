//! Somebody's place in an organisation.
//!
//! The thing that grants rights. Before this, belonging said which
//! organisations a person was shown and nothing more, while what they could
//! do came from role names in their token -- a namespace every tenant shared.
//! Roles are held here now, by id, granted inside the product.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    organisation::OrganisationId,
    role::{Role, RoleId},
    user::UserId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub struct MemberId(pub Uuid);

impl From<Uuid> for MemberId {
    fn from(value: Uuid) -> Self {
        MemberId(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Member {
    pub id: MemberId,
    pub organisation_id: OrganisationId,
    pub user_id: UserId,

    /// Who the person is, not only which row points at them. A member is a
    /// person in an organisation; a list of identifiers is not something
    /// anybody can act on, and fetching the names separately would be one
    /// request per row.
    ///
    /// A join on a primary key, on a path that already reads the row.
    pub email: String,
    pub name: String,

    /// What this person may do here. Ids rather than names: a name is what an
    /// organisation calls a role, and two organisations calling theirs the
    /// same thing is not two organisations sharing one.
    pub roles: Vec<Role>,

    pub joined_at: DateTime<Utc>,

    /// Who let them in. Absent for the owner, who joined by creating the
    /// organisation, and for anybody whose inviter has since been deleted.
    pub invited_by: Option<UserId>,
}

impl Member {
    /// Whether this member holds a given role.
    pub fn holds(&self, role_id: RoleId) -> bool {
        self.roles.iter().any(|role| role.id == role_id)
    }

    /// Every permission bit this member's roles add up to.
    ///
    /// A member with no roles holds nothing, which is not the same as not
    /// being a member: they are in the organisation, they are shown it, and
    /// they may do nothing in it until somebody grants them a role.
    pub fn permissions(&self) -> u64 {
        self.roles
            .iter()
            .fold(0, |mask, role| mask | role.permissions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(id: u128, permissions: u64) -> Role {
        Role {
            id: RoleId(Uuid::from_u128(id)),
            name: "whatever".to_string(),
            permissions,
            organisation_id: Some(OrganisationId(Uuid::from_u128(1))),
            color: None,
            created_at: Utc::now(),
        }
    }

    fn member(roles: Vec<Role>) -> Member {
        Member {
            id: MemberId(Uuid::from_u128(10)),
            organisation_id: OrganisationId(Uuid::from_u128(1)),
            user_id: UserId(Uuid::from_u128(2)),
            email: "somebody@acme.test".to_string(),
            name: "Somebody".to_string(),
            roles,
            joined_at: Utc::now(),
            invited_by: None,
        }
    }

    /// Being in and being able are two different things. A member with no
    /// roles is shown the organisation and may do nothing in it.
    #[test]
    fn a_member_with_no_roles_holds_nothing() {
        assert_eq!(member(Vec::new()).permissions(), 0);
    }

    #[test]
    fn roles_add_up_rather_than_replacing_each_other() {
        let permissions = member(vec![role(1, 0b0001), role(2, 0b0100)]).permissions();

        assert_eq!(permissions, 0b0101);
    }

    /// Two roles carrying the same bit is not a mistake worth noticing: an
    /// organisation may well grant the same thing two ways.
    #[test]
    fn the_same_bit_from_two_roles_is_held_once() {
        let permissions = member(vec![role(1, 0b0011), role(2, 0b0110)]).permissions();

        assert_eq!(permissions, 0b0111);
    }

    #[test]
    fn a_member_knows_which_roles_they_hold() {
        let member = member(vec![role(1, 0), role(2, 0)]);

        assert!(member.holds(RoleId(Uuid::from_u128(1))));
        assert!(!member.holds(RoleId(Uuid::from_u128(3))));
    }
}
