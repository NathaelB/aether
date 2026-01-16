use std::sync::OnceLock;

use aether_auth::{Identity, KeycloakAuthRepository};

use crate::{AetherService, CoreError, auth::ports::AuthService, auth::service::AuthServiceImpl};

static AUTH_ISSUER: OnceLock<String> = OnceLock::new();

pub fn set_auth_issuer(issuer: String) {
    let _ = AUTH_ISSUER.set(issuer);
}

fn auth_issuer() -> Result<&'static str, CoreError> {
    AUTH_ISSUER
        .get()
        .map(|value| value.as_str())
        .ok_or_else(|| CoreError::InternalError("Auth issuer not configured".to_string()))
}

impl AuthService for AetherService {
    async fn get_identity(&self, token: &str) -> Result<Identity, CoreError> {
        #[cfg(feature = "test-mocks")]
        if let Some(auth_service) = crate::test_mocks::auth_service() {
            return auth_service.get_identity(token).await;
        }

        let issuer = auth_issuer()?;
        let auth_repository = KeycloakAuthRepository::new(issuer.to_string(), None);
        let auth_service = AuthServiceImpl::new(std::sync::Arc::new(auth_repository));

        auth_service.get_identity(token).await
    }
}
