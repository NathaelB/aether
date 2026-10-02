use std::net::SocketAddr;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use tokio::net::TcpListener;
use tracing::info;

use crate::domain::liveness::{Clock, Liveness};

pub async fn bind(listen_addr: SocketAddr) -> std::io::Result<TcpListener> {
    TcpListener::bind(listen_addr).await
}

pub async fn serve_health<C>(listener: TcpListener, liveness: Liveness<C>) -> std::io::Result<()>
where
    C: Clock + 'static,
{
    if let Ok(listen_addr) = listener.local_addr() {
        info!(%listen_addr, "health endpoint listening");
    }
    axum::serve(listener, router(liveness)).await
}

fn router<C>(liveness: Liveness<C>) -> Router
where
    C: Clock + 'static,
{
    Router::new()
        .route("/healthz", get(healthz::<C>))
        .route("/readyz", get(readyz::<C>))
        .with_state(liveness)
}

async fn healthz<C: Clock + 'static>(
    State(liveness): State<Liveness<C>>,
) -> (StatusCode, &'static str) {
    if liveness.is_alive() {
        (StatusCode::OK, "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "sync loop stalled")
    }
}

async fn readyz<C: Clock + 'static>(
    State(liveness): State<Liveness<C>>,
) -> (StatusCode, &'static str) {
    if liveness.is_ready() {
        (StatusCode::OK, "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "no sync cycle yet")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    #[derive(Clone, Default)]
    struct FakeClock(Arc<AtomicU64>);

    impl Clock for FakeClock {
        fn elapsed(&self) -> Duration {
            Duration::from_millis(self.0.load(Ordering::SeqCst))
        }
    }

    async fn start(liveness: Liveness<FakeClock>) -> SocketAddr {
        let listener = bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(serve_health(listener, liveness));
        addr
    }

    async fn get_body(addr: SocketAddr, path: &str) -> (reqwest::StatusCode, String) {
        let response = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
        let status = response.status();
        (status, response.text().await.unwrap())
    }

    #[tokio::test]
    async fn healthz_is_ok_while_the_loop_is_turning() {
        let clock = FakeClock::default();
        let liveness = Liveness::new(clock.clone(), Duration::from_secs(15));
        liveness.record_cycle();
        let addr = start(liveness).await;

        let (status, body) = get_body(addr, "/healthz").await;

        assert_eq!(status, reqwest::StatusCode::OK);
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn healthz_is_unavailable_once_the_last_cycle_is_stale() {
        let clock = FakeClock::default();
        let liveness = Liveness::new(clock.clone(), Duration::from_secs(15));
        liveness.record_cycle();
        clock.0.fetch_add(61_000, Ordering::SeqCst);
        let addr = start(liveness).await;

        let (status, body) = get_body(addr, "/healthz").await;

        assert_eq!(status, reqwest::StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body, "sync loop stalled");
    }

    #[tokio::test]
    async fn readyz_is_unavailable_before_the_first_cycle_and_ok_after() {
        let liveness = Liveness::new(FakeClock::default(), Duration::from_secs(15));
        let addr = start(liveness.clone()).await;

        let (status, body) = get_body(addr, "/readyz").await;
        assert_eq!(status, reqwest::StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body, "no sync cycle yet");

        liveness.record_cycle();

        let (status, body) = get_body(addr, "/readyz").await;
        assert_eq!(status, reqwest::StatusCode::OK);
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn readyz_stays_ok_when_the_loop_is_stale() {
        let clock = FakeClock::default();
        let liveness = Liveness::new(clock.clone(), Duration::from_secs(15));
        liveness.record_cycle();
        clock.0.fetch_add(600_000, Ordering::SeqCst);
        let addr = start(liveness).await;

        let (status, _) = get_body(addr, "/readyz").await;

        assert_eq!(status, reqwest::StatusCode::OK);
    }

    #[tokio::test]
    async fn binding_an_address_in_use_fails() {
        let first = bind("127.0.0.1:0".parse().unwrap()).await.unwrap();

        let second = bind(first.local_addr().unwrap()).await;

        assert!(second.is_err());
    }
}
