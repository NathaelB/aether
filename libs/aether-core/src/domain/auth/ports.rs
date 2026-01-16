use aether_auth::Identity;

use crate::CoreError;

#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
pub trait AuthService: Send + Sync {
    fn get_identity(&self, token: &str)
    -> impl Future<Output = Result<Identity, CoreError>> + Send;
}

#[cfg(any(test, feature = "test-mocks"))]
impl AuthService for std::sync::Arc<MockAuthService> {
    fn get_identity(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<Identity, CoreError>> + Send {
        (**self).get_identity(token)
    }
}
