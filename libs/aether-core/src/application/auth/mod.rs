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
        let issuer = auth_issuer()?;
        let auth_repository = KeycloakAuthRepository::new(issuer.to_string(), None);
        let auth_service = AuthServiceImpl::new(std::sync::Arc::new(auth_repository));

        auth_service.get_identity(token).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_issuer_returns_value_when_set() {
        if auth_issuer().is_err() {
            set_auth_issuer("issuer-test".to_string());
        }

        let issuer = auth_issuer().expect("issuer should be configured");
        assert!(!issuer.is_empty());
    }
}
