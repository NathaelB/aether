use chrono::Utc;
use uuid::Uuid;

use crate::{
    CoreError,
    user::{
        User, UserId,
        commands::CreateUserCommand,
        ports::{UserRepository, UserService},
    },
};

#[derive(Clone)]
pub struct UserServiceImpl<U>
where
    U: UserRepository,
{
    user_repository: U,
}

impl<U> UserServiceImpl<U>
where
    U: UserRepository,
{
    pub fn new(user_repository: U) -> Self {
        Self { user_repository }
    }
}

impl<U> UserService for UserServiceImpl<U>
where
    U: UserRepository,
{
    async fn create_user(&self, command: CreateUserCommand) -> Result<User, CoreError> {
        let now = Utc::now();
        let user = User {
            id: UserId(Uuid::new_v4()),
            name: command.name,
            email: command.email,
            created_at: now,
            updated_at: now,
        };

        self.user_repository.insert(&user).await?;

        Ok(user)
    }
}
