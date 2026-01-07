use std::sync::Arc;

use aether_auth::{AuthRepository, Identity};

use crate::{CoreError, auth::ports::AuthService};

#[derive(Clone)]
pub struct AuthServiceImpl<A>
where
    A: AuthRepository,
{
    aut_repository: Arc<A>,
}

impl<A> AuthServiceImpl<A>
where
    A: AuthRepository,
{
    pub fn new(auth_repository: Arc<A>) -> Self {
        Self {
            aut_repository: auth_repository,
        }
    }
}

impl<A> AuthService for AuthServiceImpl<A>
where
    A: AuthRepository,
{
    async fn get_identity(&self, token: &str) -> Result<Identity, CoreError> {
        self.aut_repository
            .identify(token)
            .await
            .map_err(|_| CoreError::InvalidIdentity)
    }
}
