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
