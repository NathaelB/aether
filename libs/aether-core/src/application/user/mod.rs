use aether_domain::{
    CoreError,
    user::{User, commands::CreateUserCommand, ports::UserService, service::UserServiceImpl},
};

use crate::{AetherService, infrastructure::user::PostgresUserRepository};

impl UserService for AetherService {
    async fn create_user(&self, command: CreateUserCommand) -> Result<User, CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let user_repository = PostgresUserRepository::from_tx(&tx);
            let user_service = UserServiceImpl::new(user_repository);

            user_service.create_user(command).await
        };

        match result {
            Ok(user) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(user)
            }
            Err(err) => {
                super::take_transaction(&tx)
                    .await?
                    .rollback()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Err(err)
            }
        }
    }
}
