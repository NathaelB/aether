use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
    pub struct Permissions: u64 {

        const VIEW_ORGANISATION = 1 << 0;
        const MANAGE_ORGANISATION = 1 << 1;

        const VIEW_INSTANCES = 1 << 2;
        const CREATE_INSTANCES = 1 << 3;
        const MANAGE_INSTANCES = 1 << 4;
        const DELETE_INSTANCES = 1 << 5;

        const VIEW_MEMBERS = 1 << 6;
        const INVITE_MEMBERS = 1 << 7;
        const MANAGE_MEMBERS = 1 << 8;
        const KICK_MEMBERS = 1 << 9;

        const VIEW_ROLES = 1 << 10;
        const MANAGE_ROLES = 1 << 11;

        const VIEW_BILLING = 1 << 12;
        const MANAGE_BILLING = 1 << 13;

        const ADMINISTRATOR = 1 << 63;
    }
}

impl Permissions {
    pub fn can(&self, permission: Permissions) -> bool {
        self.contains(permission)
    }

    pub fn union_all(perms: &[Permissions]) -> Permissions {
        let mut result = Permissions::empty();

        for &p in perms {
            result |= p;
        }

        result
    }

    pub fn has_any(&self, permissions: &[Permissions]) -> bool {
        permissions.iter().any(|&p| self.contains(p))
    }

    pub fn to_vec(&self) -> Vec<&'static str> {
        let mut result = Vec::new();

        let flags = [
            (Permissions::VIEW_ORGANISATION, "VIEW_ORGANISATION"),
            (Permissions::MANAGE_ORGANISATION, "MANAGE_ORGANISATION"),
            (Permissions::VIEW_INSTANCES, "VIEW_INSTANCES"),
            (Permissions::CREATE_INSTANCES, "CREATE_INSTANCES"),
            (Permissions::MANAGE_INSTANCES, "MANAGE_INSTANCES"),
            (Permissions::DELETE_INSTANCES, "DELETE_INSTANCES"),
            (Permissions::VIEW_MEMBERS, "VIEW_MEMBERS"),
            (Permissions::INVITE_MEMBERS, "INVITE_MEMBERS"),
            (Permissions::MANAGE_MEMBERS, "MANAGE_MEMBERS"),
            (Permissions::KICK_MEMBERS, "KICK_MEMBERS"),
            (Permissions::VIEW_ROLES, "VIEW_ROLES"),
        ];

        for (flag, name) in flags {
            if self.contains(flag) {
                result.push(name);
            }
        }

        result
    }
}

#[macro_export]
macro_rules! require_permission {
    ($context:expr, $permission:expr) => {
        if !$context.can($permission) {
            return Err("Insufficient permissions".into());
        }
    };
}

#[macro_export]
macro_rules! require_any_permission {
    ($context:expr, $($permission:expr),+) => {
        if !$context.can_any(&[$($permission),+]) {
            return Err("Insufficient permissions".into());
        }
    };
}

#[macro_export]
macro_rules! require_all_permissions {
    ($context:expr, $($permission:expr),+) => {
        if !$context.can_all(&[$($permission),+]) {
            return Err("Insufficient permissions".into());
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::Permissions;

    #[test]
    fn test_empty_permissions() {
        let perms = Permissions::empty();
        assert!(!perms.can(Permissions::VIEW_ORGANISATION));
        assert!(!perms.can(Permissions::MANAGE_ORGANISATION));
        assert_eq!(perms.bits(), 0);
    }

    #[test]
    fn test_single_permission() {
        let perms = Permissions::VIEW_ORGANISATION;
        assert!(perms.can(Permissions::VIEW_ORGANISATION));
        assert!(!perms.can(Permissions::MANAGE_ORGANISATION));
    }

    #[test]
    fn test_multiple_permissions_with_or() {
        let perms = Permissions::VIEW_ORGANISATION | Permissions::VIEW_INSTANCES;
        assert!(perms.can(Permissions::VIEW_ORGANISATION));
        assert!(perms.can(Permissions::VIEW_INSTANCES));
        assert!(!perms.can(Permissions::MANAGE_ORGANISATION));
    }

    #[test]
    fn test_administrator_permission() {
        let perms = Permissions::ADMINISTRATOR;
        assert!(perms.can(Permissions::ADMINISTRATOR));
        // L'administrateur devrait idéalement avoir toutes les permissions
        // mais ici on teste juste le flag
        assert_eq!(perms.bits(), 1 << 63);
    }

    // === Tests de la méthode union_all ===

    #[test]
    fn test_union_all_empty() {
        let result = Permissions::union_all(&[]);
        assert_eq!(result, Permissions::empty());
    }

    #[test]
    fn test_union_all_single() {
        let result = Permissions::union_all(&[Permissions::VIEW_ORGANISATION]);
        assert!(result.can(Permissions::VIEW_ORGANISATION));
        assert!(!result.can(Permissions::MANAGE_ORGANISATION));
    }

    #[test]
    fn test_union_all_multiple() {
        let perms = [
            Permissions::VIEW_ORGANISATION,
            Permissions::VIEW_INSTANCES | Permissions::CREATE_INSTANCES,
            Permissions::VIEW_MEMBERS,
        ];
        let result = Permissions::union_all(&perms);

        assert!(result.can(Permissions::VIEW_ORGANISATION));
        assert!(result.can(Permissions::VIEW_INSTANCES));
        assert!(result.can(Permissions::CREATE_INSTANCES));
        assert!(result.can(Permissions::VIEW_MEMBERS));
        assert!(!result.can(Permissions::MANAGE_ORGANISATION));
    }

    #[test]
    fn test_union_all_overlapping() {
        let perms = [
            Permissions::VIEW_ORGANISATION | Permissions::VIEW_INSTANCES,
            Permissions::VIEW_ORGANISATION | Permissions::VIEW_MEMBERS,
        ];
        let result = Permissions::union_all(&perms);

        assert!(result.can(Permissions::VIEW_ORGANISATION));
        assert!(result.can(Permissions::VIEW_INSTANCES));
        assert!(result.can(Permissions::VIEW_MEMBERS));
    }

    // === Tests de has_any ===

    #[test]
    fn test_has_any_empty_list() {
        let perms = Permissions::VIEW_ORGANISATION;
        assert!(!perms.has_any(&[]));
    }

    #[test]
    fn test_has_any_single_match() {
        let perms = Permissions::VIEW_ORGANISATION;
        assert!(perms.has_any(&[Permissions::VIEW_ORGANISATION]));
    }

    #[test]
    fn test_has_any_no_match() {
        let perms = Permissions::VIEW_ORGANISATION;
        assert!(!perms.has_any(&[
            Permissions::MANAGE_ORGANISATION,
            Permissions::VIEW_INSTANCES
        ]));
    }

    #[test]
    fn test_has_any_partial_match() {
        let perms = Permissions::VIEW_ORGANISATION | Permissions::VIEW_INSTANCES;
        assert!(perms.has_any(&[
            Permissions::VIEW_INSTANCES,
            Permissions::MANAGE_ORGANISATION
        ]));
    }

    #[test]
    fn test_has_any_multiple_matches() {
        let perms = Permissions::VIEW_ORGANISATION
            | Permissions::VIEW_INSTANCES
            | Permissions::VIEW_MEMBERS;
        assert!(perms.has_any(&[Permissions::VIEW_ORGANISATION, Permissions::VIEW_INSTANCES]));
    }

    // === Tests de to_vec ===

    #[test]
    fn test_to_vec_empty() {
        let perms = Permissions::empty();
        let vec = perms.to_vec();
        assert_eq!(vec.len(), 0);
    }

    #[test]
    fn test_to_vec_single() {
        let perms = Permissions::VIEW_ORGANISATION;
        let vec = perms.to_vec();
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0], "VIEW_ORGANISATION");
    }

    #[test]
    fn test_to_vec_multiple() {
        let perms = Permissions::VIEW_ORGANISATION
            | Permissions::MANAGE_ORGANISATION
            | Permissions::VIEW_INSTANCES;
        let vec = perms.to_vec();
        assert_eq!(vec.len(), 3);
        assert!(vec.contains(&"VIEW_ORGANISATION"));
        assert!(vec.contains(&"MANAGE_ORGANISATION"));
        assert!(vec.contains(&"VIEW_INSTANCES"));
    }

    #[test]
    fn test_to_vec_all_view_permissions() {
        let perms = Permissions::VIEW_ORGANISATION
            | Permissions::VIEW_INSTANCES
            | Permissions::VIEW_MEMBERS
            | Permissions::VIEW_ROLES;
        let vec = perms.to_vec();
        assert_eq!(vec.len(), 4);
        assert!(vec.contains(&"VIEW_ORGANISATION"));
        assert!(vec.contains(&"VIEW_INSTANCES"));
        assert!(vec.contains(&"VIEW_MEMBERS"));
        assert!(vec.contains(&"VIEW_ROLES"));
    }

    // === Tests de bitwise operations ===

    #[test]
    fn test_bitwise_and() {
        let perms1 = Permissions::VIEW_ORGANISATION | Permissions::VIEW_INSTANCES;
        let perms2 = Permissions::VIEW_ORGANISATION | Permissions::MANAGE_ORGANISATION;
        let result = perms1 & perms2;

        assert!(result.can(Permissions::VIEW_ORGANISATION));
        assert!(!result.can(Permissions::VIEW_INSTANCES));
        assert!(!result.can(Permissions::MANAGE_ORGANISATION));
    }

    #[test]
    fn test_bitwise_or() {
        let perms1 = Permissions::VIEW_ORGANISATION;
        let perms2 = Permissions::VIEW_INSTANCES;
        let result = perms1 | perms2;

        assert!(result.can(Permissions::VIEW_ORGANISATION));
        assert!(result.can(Permissions::VIEW_INSTANCES));
    }

    #[test]
    fn test_bitwise_xor() {
        let perms1 = Permissions::VIEW_ORGANISATION | Permissions::VIEW_INSTANCES;
        let perms2 = Permissions::VIEW_ORGANISATION | Permissions::MANAGE_ORGANISATION;
        let result = perms1 ^ perms2;

        // XOR: seuls les bits différents restent
        assert!(!result.can(Permissions::VIEW_ORGANISATION)); // présent dans les deux
        assert!(result.can(Permissions::VIEW_INSTANCES)); // seulement dans perms1
        assert!(result.can(Permissions::MANAGE_ORGANISATION)); // seulement dans perms2
    }

    #[test]
    fn test_bitwise_not() {
        let perms = Permissions::VIEW_ORGANISATION;
        let result = !perms;

        // NOT inverse tous les bits
        assert!(!result.can(Permissions::VIEW_ORGANISATION));
        assert!(result.can(Permissions::MANAGE_ORGANISATION));
        assert!(result.can(Permissions::VIEW_INSTANCES));
    }

    // === Tests de sérialisation/désérialisation ===

    #[test]
    fn test_serialize_deserialize() {
        let perms = Permissions::VIEW_ORGANISATION | Permissions::MANAGE_INSTANCES;

        let serialized = serde_json::to_string(&perms).unwrap();
        let deserialized: Permissions = serde_json::from_str(&serialized).unwrap();

        assert_eq!(perms, deserialized);
    }

    #[test]
    fn test_serialize_empty() {
        let perms = Permissions::empty();
        let serialized = serde_json::to_string(&perms).unwrap();
        let deserialized: Permissions = serde_json::from_str(&serialized).unwrap();

        assert_eq!(perms, deserialized);
        assert_eq!(deserialized.bits(), 0);
    }

    #[test]
    fn test_serialize_administrator() {
        let perms = Permissions::ADMINISTRATOR;
        let serialized = serde_json::to_string(&perms).unwrap();
        let deserialized: Permissions = serde_json::from_str(&serialized).unwrap();

        assert_eq!(perms, deserialized);
        assert!(deserialized.can(Permissions::ADMINISTRATOR));
    }

    // === Tests de cas d'usage réels ===

    #[test]
    fn test_basic_user_permissions() {
        // Un utilisateur basic peut voir mais pas gérer
        let perms = Permissions::VIEW_ORGANISATION
            | Permissions::VIEW_INSTANCES
            | Permissions::VIEW_MEMBERS;

        assert!(perms.can(Permissions::VIEW_ORGANISATION));
        assert!(perms.can(Permissions::VIEW_INSTANCES));
        assert!(!perms.can(Permissions::MANAGE_ORGANISATION));
        assert!(!perms.can(Permissions::DELETE_INSTANCES));
    }

    #[test]
    fn test_moderator_permissions() {
        // Un modérateur peut gérer les membres mais pas l'organisation
        let perms = Permissions::VIEW_ORGANISATION
            | Permissions::VIEW_MEMBERS
            | Permissions::INVITE_MEMBERS
            | Permissions::MANAGE_MEMBERS
            | Permissions::KICK_MEMBERS;

        assert!(perms.can(Permissions::MANAGE_MEMBERS));
        assert!(perms.can(Permissions::KICK_MEMBERS));
        assert!(!perms.can(Permissions::MANAGE_ORGANISATION));
        assert!(!perms.can(Permissions::MANAGE_BILLING));
    }

    #[test]
    fn test_admin_permissions() {
        // Un admin a toutes les permissions sauf ADMINISTRATOR
        let perms = Permissions::VIEW_ORGANISATION
            | Permissions::MANAGE_ORGANISATION
            | Permissions::VIEW_INSTANCES
            | Permissions::CREATE_INSTANCES
            | Permissions::MANAGE_INSTANCES
            | Permissions::DELETE_INSTANCES
            | Permissions::VIEW_MEMBERS
            | Permissions::INVITE_MEMBERS
            | Permissions::MANAGE_MEMBERS
            | Permissions::KICK_MEMBERS
            | Permissions::VIEW_ROLES
            | Permissions::MANAGE_ROLES
            | Permissions::VIEW_BILLING
            | Permissions::MANAGE_BILLING;

        assert!(perms.can(Permissions::MANAGE_ORGANISATION));
        assert!(perms.can(Permissions::DELETE_INSTANCES));
        assert!(perms.can(Permissions::MANAGE_BILLING));
        assert!(!perms.can(Permissions::ADMINISTRATOR));
    }

    #[test]
    fn test_instance_manager_permissions() {
        // Un gestionnaire d'instances avec permissions spécifiques
        let perms = Permissions::VIEW_ORGANISATION
            | Permissions::VIEW_INSTANCES
            | Permissions::CREATE_INSTANCES
            | Permissions::MANAGE_INSTANCES
            | Permissions::DELETE_INSTANCES;

        assert!(perms.can(Permissions::VIEW_INSTANCES));
        assert!(perms.can(Permissions::CREATE_INSTANCES));
        assert!(perms.can(Permissions::DELETE_INSTANCES));
        assert!(!perms.can(Permissions::MANAGE_MEMBERS));
        assert!(!perms.can(Permissions::MANAGE_BILLING));
    }

    // === Tests de scénarios multi-rôles ===

    #[test]
    fn test_multiple_roles_union() {
        // Simule un user avec plusieurs rôles (viewer + instance manager)
        let role1 = Permissions::VIEW_ORGANISATION | Permissions::VIEW_MEMBERS;
        let role2 = Permissions::VIEW_INSTANCES | Permissions::CREATE_INSTANCES;

        let effective_perms = Permissions::union_all(&[role1, role2]);

        assert!(effective_perms.can(Permissions::VIEW_ORGANISATION));
        assert!(effective_perms.can(Permissions::VIEW_MEMBERS));
        assert!(effective_perms.can(Permissions::VIEW_INSTANCES));
        assert!(effective_perms.can(Permissions::CREATE_INSTANCES));
        assert!(!effective_perms.can(Permissions::MANAGE_ORGANISATION));
    }

    #[test]
    fn test_permission_escalation_check() {
        // Vérifier qu'on ne peut pas escalader sans MANAGE_ROLES
        let user_perms = Permissions::VIEW_ORGANISATION | Permissions::VIEW_ROLES;

        assert!(user_perms.can(Permissions::VIEW_ROLES));
        assert!(!user_perms.can(Permissions::MANAGE_ROLES));
    }

    // === Tests de bits values ===

    #[test]
    fn test_permission_bit_values() {
        // Vérifier que les valeurs des bits sont correctes
        assert_eq!(Permissions::VIEW_ORGANISATION.bits(), 1 << 0);
        assert_eq!(Permissions::MANAGE_ORGANISATION.bits(), 1 << 1);
        assert_eq!(Permissions::VIEW_INSTANCES.bits(), 1 << 2);
        assert_eq!(Permissions::ADMINISTRATOR.bits(), 1 << 63);
    }

    #[test]
    fn test_no_permission_overlap() {
        // Vérifier qu'il n'y a pas de chevauchement de bits
        let all_perms = [
            Permissions::VIEW_ORGANISATION,
            Permissions::MANAGE_ORGANISATION,
            Permissions::VIEW_INSTANCES,
            Permissions::CREATE_INSTANCES,
            Permissions::MANAGE_INSTANCES,
            Permissions::DELETE_INSTANCES,
            Permissions::VIEW_MEMBERS,
            Permissions::INVITE_MEMBERS,
            Permissions::MANAGE_MEMBERS,
            Permissions::KICK_MEMBERS,
            Permissions::VIEW_ROLES,
            Permissions::MANAGE_ROLES,
            Permissions::VIEW_BILLING,
            Permissions::MANAGE_BILLING,
            Permissions::ADMINISTRATOR,
        ];

        for (i, &perm1) in all_perms.iter().enumerate() {
            for &perm2 in all_perms.iter().skip(i + 1) {
                assert_ne!(
                    perm1.bits(),
                    perm2.bits(),
                    "Permissions have overlapping bits"
                );
            }
        }
    }
}
