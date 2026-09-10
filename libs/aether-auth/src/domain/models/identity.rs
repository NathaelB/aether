use serde::{Deserialize, Serialize};

use crate::domain::models::{claims::Claims, client::Client, user::User};

/// Realm role that marks an identity as operating this installation.
pub const OPERATOR_ROLE: &str = "aether-operator";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Identity {
    User(User),
    Client(Client),
}

impl Identity {
    pub fn id(&self) -> &str {
        match self {
            Identity::User(u) => &u.id,
            Identity::Client(c) => &c.id,
        }
    }

    pub fn is_user(&self) -> bool {
        matches!(self, Identity::User(_))
    }

    pub fn is_client(&self) -> bool {
        matches!(self, Identity::Client(_))
    }

    /// Whether this identity operates the installation, as opposed to using it.
    ///
    /// Installation-wide rather than organisation-scoped: the permissions in
    /// `aether-permission` say what a member may do inside their organisation,
    /// which is a different question from whether someone may see the
    /// infrastructure every organisation runs on.
    pub fn is_operator(&self) -> bool {
        self.roles().iter().any(|role| role == OPERATOR_ROLE)
    }

    pub fn username(&self) -> &str {
        match self {
            Identity::User(u) => &u.username,
            Identity::Client(c) => &c.client_id,
        }
    }

    pub fn roles(&self) -> &[String] {
        match self {
            Identity::User(u) => &u.roles,
            Identity::Client(c) => &c.roles,
        }
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles().iter().any(|r| r == role)
    }
}

impl From<Claims> for Identity {
    fn from(claims: Claims) -> Self {
        let roles = claims.realm_roles().to_vec();

        if let Some(client_id) = claims.client_id {
            Identity::Client(Client {
                id: claims.sub.0,
                client_id,
                roles,
                scopes: Vec::new(),
            })
        } else {
            Identity::User(User {
                id: claims.sub.0.clone(),
                email: claims.email,
                name: claims.name,
                roles,
                username: claims.preferred_username.unwrap_or(claims.sub.0),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    /// Roles used to be dropped on the floor here: the field existed and was
    /// always empty, so any role check would have silently refused everyone.
    #[test]
    fn realm_roles_reach_the_identity() {
        let claims: Claims = serde_json::from_value(json!({
            "sub": "01234567-89ab-cdef-0123-456789abcdef",
            "iss": "https://id.example/realms/aether",
            "scope": "openid",
            "preferred_username": "nathael",
            "realm_access": { "roles": ["aether-operator", "default-roles-aether"] }
        }))
        .expect("claims parse");

        let identity = Identity::from(claims);

        assert!(identity.roles().contains(&"aether-operator".to_string()));
        assert!(identity.is_operator());
    }

    #[test]
    fn an_identity_without_the_role_is_not_an_operator() {
        let claims: Claims = serde_json::from_value(json!({
            "sub": "01234567-89ab-cdef-0123-456789abcdef",
            "iss": "https://id.example/realms/aether",
            "scope": "openid",
            "preferred_username": "customer",
            "realm_access": { "roles": ["default-roles-aether"] }
        }))
        .expect("claims parse");

        assert!(!Identity::from(claims).is_operator());
    }

    /// A token from an identity provider that sends no `realm_access` at all
    /// must parse rather than fail closed on deserialisation.
    #[test]
    fn a_token_without_realm_access_still_parses() {
        let claims: Claims = serde_json::from_value(json!({
            "sub": "01234567-89ab-cdef-0123-456789abcdef",
            "iss": "https://id.example/realms/aether",
            "scope": "openid",
            "preferred_username": "customer"
        }))
        .expect("claims parse");

        assert!(claims.realm_roles().is_empty());
        assert!(!Identity::from(claims).is_operator());
    }

    use crate::domain::models::{
        claims::{Audience, Claims, RealmAccess},
        identity::Identity,
    };

    fn create_user_claims() -> Claims {
        Claims {
            sub: crate::domain::models::claims::Subject("user-123".to_string()),
            iss: "https://auth.ferriscord.com".to_string(),
            aud: Some(Audience::Single("ferriscord-api".to_string())),
            email: Some("john.doe@example.com".to_string()),
            email_verified: Some(true),
            exp: None,
            name: Some("John Doe".to_string()),
            preferred_username: Some("johndoe".to_string()),
            given_name: Some("John".to_string()),
            family_name: Some("Doe".to_string()),
            scope: "openid profile email".to_string(),
            client_id: None,
            realm_access: Some(RealmAccess {
                roles: ["user", "moderator"]
                    .iter()
                    .map(|r: &&str| r.to_string())
                    .collect(),
            }),
            extra: serde_json::Map::new(),
        }
    }

    fn create_service_account_claims() -> Claims {
        Claims {
            sub: crate::domain::models::claims::Subject("service-123".to_string()),
            iss: "https://auth.ferriscord.com".to_string(),
            aud: Some(Audience::Single("ferriscord-api".to_string())),
            email: None,
            email_verified: Some(false),
            name: None,
            exp: None,
            preferred_username: Some("service-account-bot".to_string()),
            given_name: None,
            family_name: None,
            scope: "admin:all read:users write:messages".to_string(),
            client_id: Some("ferriscord-bot".to_string()),
            realm_access: Some(RealmAccess {
                roles: ["service", "bot"]
                    .iter()
                    .map(|r: &&str| r.to_string())
                    .collect(),
            }),
            extra: serde_json::Map::new(),
        }
    }

    #[test]
    fn test_claims_to_identity_user() {
        let claims = create_user_claims();
        let identity: Identity = claims.into();

        match identity {
            Identity::User(user) => {
                assert_eq!(user.id, "user-123");
                assert_eq!(user.username, "johndoe");
                assert_eq!(user.email, Some("john.doe@example.com".to_string()));
                assert_eq!(user.name, Some("John Doe".to_string()));
            }
            Identity::Client(_) => panic!("Expected User, got Client"),
        }
    }

    #[test]
    fn test_claims_to_identity_service_account() {
        let claims = create_service_account_claims();
        let identity: Identity = claims.into();

        match identity {
            Identity::Client(client) => {
                assert_eq!(client.id, "service-123");
                assert_eq!(client.client_id, "ferriscord-bot");
            }
            Identity::User(_) => panic!("Expected Client, got User"),
        }
    }

    #[test]
    fn test_identity_accessors_for_user() {
        let claims = create_user_claims();
        let identity: Identity = claims.into();

        assert!(identity.is_user());
        assert!(!identity.is_client());
        assert_eq!(identity.id(), "user-123");
        assert_eq!(identity.username(), "johndoe");
        assert_eq!(identity.roles(), ["user", "moderator"]);
        assert!(!identity.has_role("admin"));
    }

    #[test]
    fn test_identity_accessors_for_client() {
        let claims = create_service_account_claims();
        let identity: Identity = claims.into();

        assert!(identity.is_client());
        assert!(!identity.is_user());
        assert_eq!(identity.id(), "service-123");
        assert_eq!(identity.username(), "ferriscord-bot");
        assert_eq!(identity.roles(), ["service", "bot"]);
        assert!(identity.has_role("service"));
    }
}
