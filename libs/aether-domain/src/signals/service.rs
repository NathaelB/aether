use aether_auth::Identity;
use chrono::{DateTime, Utc};

use crate::{
    CoreError,
    platform::{PlatformRight, ports::PlatformPolicy},
};

use super::{
    Signal, SignalKind, SignalSubject,
    ports::{SignalListPage, SignalRepository},
};

pub struct SignalServiceImpl<R, P>
where
    R: SignalRepository,
    P: PlatformPolicy,
{
    signals: R,
    policy: P,
}

impl<R, P> SignalServiceImpl<R, P>
where
    R: SignalRepository,
    P: PlatformPolicy,
{
    pub fn new(signals: R, policy: P) -> Self {
        Self { signals, policy }
    }

    pub async fn write_signal(&self, signal: Signal) -> Result<(), CoreError> {
        self.signals.write(signal).await
    }

    pub async fn close_signal(&self, dedup_key: &str, at: DateTime<Utc>) -> Result<(), CoreError> {
        self.signals.close(dedup_key, at).await
    }

    pub async fn list_open_signals(
        &self,
        identity: Identity,
        kind_filter: Option<SignalKind>,
        subject_filter: Option<SignalSubject>,
        limit: usize,
        cursor: Option<String>,
    ) -> Result<SignalListPage, CoreError> {
        self.policy
            .require(identity, PlatformRight::ViewEstate)
            .await?;

        self.signals
            .list_open(kind_filter, subject_filter, limit, cursor)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    use crate::{
        dataplane::value_objects::DataPlaneId,
        platform::ports::PlatformPolicy,
        signals::{SignalId, SignalKind, SignalSubject},
    };

    struct AllowingPolicy;

    impl PlatformPolicy for AllowingPolicy {
        async fn require(
            &self,
            _identity: Identity,
            _right: crate::platform::PlatformRight,
        ) -> Result<(), CoreError> {
            Ok(())
        }

        async fn rights_of(
            &self,
            _identity: Identity,
        ) -> Result<crate::platform::PlatformRights, CoreError> {
            Ok(crate::platform::PlatformRights::everything())
        }
    }

    #[tokio::test]
    async fn the_service_writes_a_signal() {
        let mut signals = super::super::ports::MockSignalRepository::new();
        signals
            .expect_write()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = SignalServiceImpl::new(signals, AllowingPolicy);
        let signal = Signal::open(
            SignalId(Uuid::new_v4()),
            SignalKind::DataplaneHeartbeatStale,
            SignalSubject::Dataplane {
                id: DataPlaneId(Uuid::new_v4()),
            },
            "key-1".to_string(),
            "Dataplane is stale".to_string(),
            Utc::now(),
        );

        assert!(service.write_signal(signal).await.is_ok());
    }

    #[tokio::test]
    async fn the_service_closes_a_signal() {
        let mut signals = super::super::ports::MockSignalRepository::new();
        signals
            .expect_close()
            .times(1)
            .withf(|key, _| key == "key-1")
            .returning(|_, _| Box::pin(async { Ok(()) }));

        let service = SignalServiceImpl::new(signals, AllowingPolicy);

        assert!(service.close_signal("key-1", Utc::now()).await.is_ok());
    }

    #[tokio::test]
    async fn the_service_lists_open_signals_when_authorized() {
        let mut signals = super::super::ports::MockSignalRepository::new();
        signals.expect_list_open().times(1).returning(|_, _, _, _| {
            Box::pin(async {
                Ok(SignalListPage {
                    signals: vec![],
                    next_cursor: None,
                })
            })
        });

        let service = SignalServiceImpl::new(signals, AllowingPolicy);

        let test_identity = aether_auth::Identity::User(aether_auth::User {
            id: "test-user".to_string(),
            username: "test".to_string(),
            email: None,
            name: None,
            roles: vec![],
        });

        let result = service
            .list_open_signals(test_identity, None, None, 10, None)
            .await;

        assert!(result.is_ok());
    }
}
