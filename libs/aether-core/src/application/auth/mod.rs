use aether_auth::Identity;

use crate::{AetherService, CoreError, auth::ports::AuthService};

impl AuthService for AetherService {
    async fn get_identity(&self, token: &str) -> Result<Identity, CoreError> {
        self.auth_service.get_identity(token).await
    }
}
