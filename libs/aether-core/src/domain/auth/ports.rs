use aether_auth::Identity;

use crate::CoreError;

pub trait AuthService: Send + Sync {
    fn get_identity(&self, token: &str)
    -> impl Future<Output = Result<Identity, CoreError>> + Send;
}
