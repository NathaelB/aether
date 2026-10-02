use aether_auth::{Identity, User};
use aether_domain::CoreError;
use aether_domain::platform::{PlatformRight, PlatformRights, ports::PlatformPolicy};

#[derive(Debug, Clone)]
pub struct HeldRights(PlatformRights);

impl HeldRights {
    pub fn of(rights: &[PlatformRight]) -> Self {
        Self(PlatformRights::of(rights.iter().copied()))
    }
}

impl PlatformPolicy for HeldRights {
    async fn require(&self, _: Identity, right: PlatformRight) -> Result<(), CoreError> {
        if self.0.holds(right) {
            return Ok(());
        }
        Err(CoreError::MissingPlatformRight {
            right: right.to_string(),
        })
    }

    async fn rights_of(&self, _: Identity) -> Result<PlatformRights, CoreError> {
        Ok(self.0.clone())
    }
}

pub fn caller() -> Identity {
    Identity::User(User {
        id: "operator-1".to_string(),
        username: "operator".to_string(),
        email: None,
        name: None,
        roles: vec![],
    })
}
