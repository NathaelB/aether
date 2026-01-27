use crate::{
    CoreError,
    user::{User, commands::CreateUserCommand},
};

pub trait UserService: Send + Sync {
    fn create_user(
        &self,
        command: CreateUserCommand,
    ) -> impl Future<Output = Result<User, CoreError>> + Send;
}

pub trait UserRepository: Send + Sync {
    fn insert(&self, user: &User) -> impl Future<Output = Result<(), CoreError>> + Send;
}
