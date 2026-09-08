use std::sync::Arc;

use sqlx::{PgPool, Postgres, Transaction};
use tokio::sync::{Mutex, MutexGuard};

pub type PgTransaction<'t> = tokio::sync::Mutex<Option<Transaction<'t, Postgres>>>;

pub enum PgExecutor<'e, 't> {
    Pool(&'e PgPool),
    Tx(&'e PgTransaction<'t>),
}

impl<'e> PgExecutor<'e, 'e> {
    pub fn from_pool(pool: &'e PgPool) -> Self {
        Self::Pool(pool)
    }
}

impl<'e, 't> PgExecutor<'e, 't> {
    pub fn from_tx(tx: &'e PgTransaction<'t>) -> Self {
        Self::Tx(tx)
    }
}

/// Cloneable handle over a live transaction, so several repositories can share
/// one without juggling exclusive `&mut` borrows.
///
/// This is what removes the `PgExecutor::{Pool, Tx}` split. That split forced
/// every repository method to `match` on its executor and write the same SQL
/// twice — 38 duplicated statements across the six repositories. A repository
/// that only ever holds a transaction writes each query once.
///
/// Each repository takes its own clone and locks the inner transaction for the
/// duration of a single query. Contention is effectively zero: a use case runs
/// sequentially on one task. The mutex is there to satisfy the borrow checker
/// while keeping the hexagonal split intact — the domain service never learns
/// that a transaction exists.
pub struct SharedTx<'tx> {
    inner: Arc<Mutex<&'tx mut Transaction<'static, Postgres>>>,
}

impl<'tx> SharedTx<'tx> {
    fn new(tx: &'tx mut Transaction<'static, Postgres>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(tx)),
        }
    }

    pub async fn lock(&self) -> MutexGuard<'_, &'tx mut Transaction<'static, Postgres>> {
        self.inner.lock().await
    }
}

impl Clone for SharedTx<'_> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// Runs `work` inside one transaction, committing on `Ok` and rolling back on
/// `Err`.
///
/// A rollback failure is deliberately swallowed: the caller's error is the one
/// worth reporting, and reporting the rollback instead would hide why the work
/// failed in the first place. The transaction is dropped either way, which
/// rolls it back at the connection level.
pub async fn with_tx<F, T, E>(pool: &PgPool, work: F) -> Result<T, E>
where
    F: AsyncFnOnce(SharedTx<'_>) -> Result<T, E>,
    E: From<sqlx::Error>,
{
    let mut tx = pool.begin().await.map_err(E::from)?;

    let result = {
        let shared = SharedTx::new(&mut tx);
        work(shared).await
        // every SharedTx clone drops here, releasing the &mut borrow on `tx`
    };

    match result {
        Ok(value) => {
            tx.commit().await.map_err(E::from)?;
            Ok(value)
        }
        Err(err) => {
            let _ = tx.rollback().await;
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn from_pool_returns_pool_variant() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://user:pass@localhost:5432/db")
            .expect("valid database url");

        let executor = PgExecutor::from_pool(&pool);
        match executor {
            PgExecutor::Pool(inner) => assert!(std::ptr::eq(inner, &pool)),
            PgExecutor::Tx(_) => panic!("expected pool variant"),
        }
    }

    #[derive(Debug)]
    struct TestError(String);

    impl From<sqlx::Error> for TestError {
        fn from(error: sqlx::Error) -> Self {
            Self(error.to_string())
        }
    }

    /// `with_tx` cannot open a transaction against an unreachable database, so
    /// this covers the one path reachable without a live server: the failure to
    /// begin is converted through `E::from` rather than panicking, and the
    /// caller's closure is never run.
    #[tokio::test]
    async fn with_tx_converts_a_failure_to_begin() {
        let pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(50))
            .connect_lazy("postgres://user:pass@127.0.0.1:1/db")
            .expect("valid database url");

        let ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = ran.clone();

        let result: Result<(), TestError> = with_tx(&pool, async |_tx| {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .await;

        let Err(TestError(message)) = result else {
            panic!("an unreachable database must not commit");
        };
        assert!(
            !message.is_empty(),
            "the sqlx error must survive the conversion"
        );
        assert!(
            !ran.load(std::sync::atomic::Ordering::SeqCst),
            "the closure must not run when the transaction never opened"
        );
    }

    #[test]
    fn from_tx_returns_tx_variant() {
        let tx: PgTransaction<'static> = tokio::sync::Mutex::new(None);

        let executor = PgExecutor::from_tx(&tx);
        match executor {
            PgExecutor::Tx(inner) => assert!(std::ptr::eq(inner, &tx)),
            PgExecutor::Pool(_) => panic!("expected tx variant"),
        }
    }
}
