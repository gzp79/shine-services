use crate::db::{DBConfig, DBError};
use bb8::{ManageConnection, Pool as BB8Pool, PooledConnection, RunError, State};
use bb8_postgres::PostgresConnectionManager;
use tokio_postgres_rustls::MakeRustlsConnect;
use tokio_rustls::rustls;

pub type DBPoolState = State;
pub type DBConnection = PostgresConnectionManager<MakeRustlsConnect>;
pub type DBConnectionError = RunError<<DBConnection as ManageConnection>::Error>;
type ConnectionPool = BB8Pool<DBConnection>;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./sql_migrations");
}

#[derive(Clone)]
pub struct DBPool {
    pool: ConnectionPool,
}

impl DBPool {
    pub async fn new(config: &DBConfig) -> Result<Self, DBError> {
        let tls_config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth();
        let tls = MakeRustlsConnect::new(tls_config);

        let manager = PostgresConnectionManager::new_from_stringlike(&config.connection_string, tls)?;
        let pool = bb8::Pool::builder()
            .max_size(10) // Set the maximum number of connections in the pool
            .build(manager)
            .await?;

        let pool = Self { pool };
        pool.migrate().await?;
        Ok(pool)
    }

    pub fn state(&self) -> DBPoolState {
        self.pool.state()
    }

    pub async fn get(&self) -> Result<PooledConnection<'_, DBConnection>, DBError> {
        Ok(self.pool.get().await?)
    }

    async fn migrate(&self) -> Result<(), DBError> {
        let mut backend = self.pool.get().await?;
        log::info!("migrations: {:?}", embedded::migrations::runner().get_migrations());
        embedded::migrations::runner().run_async(&mut *backend).await?;
        Ok(())
    }
}
