use sqlx::{PgPool, Postgres, Transaction};

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
