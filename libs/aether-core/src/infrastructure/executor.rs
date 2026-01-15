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
