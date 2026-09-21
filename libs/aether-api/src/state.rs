use std::sync::Arc;

use aether_core::{AetherConfig, AetherService, create_service};
use tracing::warn;

use crate::{
    args::Args, certificate::KubeCertificateSource, errors::ApiError,
    quickwit::QuickwitLogSearchIndex,
};

#[derive(Clone)]
pub struct AppState {
    #[allow(unused)]
    pub args: Arc<Args>,

    #[allow(unused)]
    pub service: AetherService,

    /// Where the current certificate can be read from, when this
    /// installation was given one. `None` the same way a missing
    /// `DnsProvider` is: nothing that reads this fails, it simply has
    /// nothing to distribute.
    pub certificate_source: Option<Arc<KubeCertificateSource>>,

    /// Where log search answers from, when this installation runs Quickwit.
    /// `None` for an installation that has not configured `--quickwit-url`:
    /// the search endpoint refuses plainly rather than the request failing to
    /// connect somewhere.
    pub quickwit_search: Option<Arc<QuickwitLogSearchIndex>>,
}

pub async fn state(args: Arc<Args>) -> Result<AppState, ApiError> {
    let config: AetherConfig = AetherConfig::from(args.as_ref().clone());

    let service = create_service(config)
        .await
        .map_err(|e| ApiError::InternalServerError {
            reason: e.to_string(),
        })?
        // The one administrative capability the control plane holds on the
        // realm. Given here rather than read from the environment deeper in,
        // so an installation that has none is visible in the wiring.
        .administering(args.realm.admin())
        .with_domain(args.ovh.domain());

    // Best-effort, like every other optional integration here: a cluster
    // this pod cannot reach, or a Secret that never turns up, means no
    // certificate is distributed -- not that the control plane fails to
    // start.
    let certificate_source = match args.certificate.configured() {
        Some((name, namespace)) => match KubeCertificateSource::from_env(name, namespace).await {
            Ok(source) => Some(Arc::new(source)),
            Err(error) => {
                warn!(%error, "the certificate source could not be built: no certificate will be distributed");
                None
            }
        },
        None => None,
    };

    let quickwit_search = args
        .quickwit
        .configured()
        .map(|url| Arc::new(QuickwitLogSearchIndex::new(url)));

    Ok(AppState {
        args,
        service,
        certificate_source,
        quickwit_search,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{Args, DatabaseArgs};
    use std::sync::Arc;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn state_returns_error_on_invalid_db() {
        let args = Args {
            db: DatabaseArgs {
                host: "127.0.0.1".to_string(),
                port: 1,
                ..DatabaseArgs::default()
            },
            ..Args::default()
        };

        let result = timeout(Duration::from_millis(200), state(Arc::new(args))).await;
        assert!(matches!(result, Ok(Err(ApiError::InternalServerError { .. }))) || result.is_err());
    }
}
