use aether_auth::Identity;
use aether_domain::signals::{
    Signal, SignalKind, SignalSubject,
    ports::{SignalListPage, SignalRepository},
};
use aether_macros::transactional;
use chrono::{DateTime, Utc};

use crate::{AetherService, CoreError, policy::PlatformRightsPolicy};

impl AetherService {
    #[transactional(signal)]
    pub async fn write_signal(&self, signal: Signal) -> Result<(), CoreError> {
        signal_repository.write(signal).await
    }

    #[transactional(signal)]
    pub async fn close_signal(&self, dedup_key: &str, at: DateTime<Utc>) -> Result<(), CoreError> {
        signal_repository.close(dedup_key, at).await
    }

    #[transactional(signal)]
    pub async fn list_open_signals(
        &self,
        identity: Identity,
        kind_filter: Option<SignalKind>,
        subject_filter: Option<SignalSubject>,
        limit: usize,
        cursor: Option<String>,
    ) -> Result<SignalListPage, CoreError> {
        let service = aether_domain::signals::service::SignalServiceImpl::new(
            signal_repository,
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
        );

        service
            .list_open_signals(identity, kind_filter, subject_filter, limit, cursor)
            .await
    }
}
