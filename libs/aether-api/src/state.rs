use std::sync::Arc;

use aether_core::{AetherConfig, AetherService, create_service};

use crate::{args::Args, errors::ApiError};

#[derive(Clone)]
pub struct AppState {
    #[allow(unused)]
    pub args: Arc<Args>,

    #[allow(unused)]
    pub service: AetherService,
}

pub async fn state(args: Arc<Args>) -> Result<AppState, ApiError> {
    let config: AetherConfig = AetherConfig::from(args.as_ref().clone());

    let service = create_service(config)
        .await
        .map_err(|e| ApiError::InternalServerError {
            reason: e.to_string(),
        })?;

    Ok(AppState { args, service })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{Args, AuthArgs, DatabaseArgs, LogArgs, ServerArgs};
    use std::sync::Arc;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn state_returns_error_on_invalid_db() {
        let args = Args {
            log: LogArgs::default(),
            db: DatabaseArgs {
                host: "127.0.0.1".to_string(),
                port: 1,
                ..DatabaseArgs::default()
            },
            auth: AuthArgs {
                issuer: "http://issuer.test".to_string(),
            },
            server: ServerArgs::default(),
        };

        let result = timeout(Duration::from_millis(200), state(Arc::new(args))).await;
        assert!(matches!(result, Ok(Err(ApiError::InternalServerError { .. }))) || result.is_err());
    }
}
