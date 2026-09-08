use aether_domain::{
    CoreError,
    user::{User, commands::CreateUserCommand, ports::UserService, service::UserServiceImpl},
};
use aether_macros::transactional;

use crate::AetherService;

impl UserService for AetherService {
    #[transactional(user)]
    async fn create_user(&self, command: CreateUserCommand) -> Result<User, CoreError> {
        UserServiceImpl::new(user_repository)
            .create_user(command)
            .await
    }
}
