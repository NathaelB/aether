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

#[cfg(test)]
mod tests {
    use super::*;
    use aether_auth::{AuthError, AuthRepository, Identity, User};

    struct FakeAuthRepository {
        identity: Option<Identity>,
        fail: bool,
    }

    impl FakeAuthRepository {
        fn success(identity: Identity) -> Self {
            Self {
                identity: Some(identity),
                fail: false,
            }
        }

        fn failure() -> Self {
            Self {
                identity: None,
                fail: true,
            }
        }
    }

    impl AuthRepository for FakeAuthRepository {
        fn validate_token(
            &self,
            _token: &str,
        ) -> impl Future<Output = Result<aether_auth::Claims, AuthError>> + Send {
            Box::pin(async {
                Err(AuthError::InvalidToken {
                    message: "invalid".to_string(),
                })
            })
        }

        fn identify(
            &self,
            _token: &str,
        ) -> impl Future<Output = Result<Identity, AuthError>> + Send {
            let result = if self.fail {
                Err(AuthError::InvalidToken {
                    message: "invalid".to_string(),
                })
            } else {
                Ok(self.identity.clone().expect("identity required"))
            };

            Box::pin(async move { result })
        }
    }

    #[tokio::test]
    async fn get_identity_returns_identity() {
        let identity = Identity::User(User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec!["admin".to_string()],
        });
        let service = AuthServiceImpl::new(Arc::new(FakeAuthRepository::success(identity.clone())));

        let result = service.get_identity("token").await;
        assert_eq!(result.unwrap(), identity);
    }

    #[tokio::test]
    async fn get_identity_maps_error() {
        let service = AuthServiceImpl::new(Arc::new(FakeAuthRepository::failure()));
        let result = service.get_identity("token").await;

        assert!(matches!(result, Err(CoreError::InvalidIdentity)));
    }
}
