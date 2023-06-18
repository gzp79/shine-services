use std::str::FromStr;

use crate::db::{DBConfig, DBError};
use bb8::{ManageConnection, Pool as BB8Pool, RunError};
use bb8_postgres::PostgresConnectionManager;
use bb8_redis::RedisConnectionManager;
use shine_service::service::cacerts;
use tokio_postgres::Config as PGConfig;
use tokio_postgres_rustls::MakeRustlsConnect;
use tokio_rustls::rustls;

pub type PGConnection = PostgresConnectionManager<MakeRustlsConnect>;
pub type PGConnectionError = RunError<<PGConnection as ManageConnection>::Error>;
pub type PGConnectionPool = BB8Pool<PGConnection>;

pub type RedisConnection = RedisConnectionManager;
pub type RedisConnectionError = RunError<<RedisConnection as ManageConnection>::Error>;
pub type RedisConnectionPool = BB8Pool<RedisConnection>;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./sql_migrations");
}

#[derive(Clone)]
pub struct DBPool {
    pub postgres: PGConnectionPool,
    pub redis: RedisConnectionPool,
}

impl DBPool {
    pub async fn new(config: &DBConfig) -> Result<Self, DBError> {
        //todo: make tls optional (from feature as tls is a property of the connection type, see NoTls). 
        //      It can be disabled when running in cloud on a virtual network.
        let tls_config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(cacerts::get_root_cert_store())
            .with_no_client_auth();
        let tls = MakeRustlsConnect::new(tls_config);

        let pg_config = PGConfig::from_str(&config.sql_cns)?;
        log::debug!("Postgresql config: {pg_config:#?}");
        let postgres_manager = PostgresConnectionManager::new(pg_config, tls);
        let postgres = bb8::Pool::builder()
            .max_size(10) // Set the maximum number of connections in the pool
            .build(postgres_manager)
            .await?;

        let redis_manager = RedisConnectionManager::new(config.redis_cns.as_str())?;
        let redis = bb8::Pool::builder()
            .max_size(10) // Set the maximum number of connections in the pool
            .build(redis_manager)
            .await?;

        let pool = Self { postgres, redis };
        pool.migrate().await?;
        Ok(pool)
    }

    async fn migrate(&self) -> Result<(), DBError> {
        let mut backend = self.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        log::info!("migrations: {:#?}", embedded::migrations::runner().get_migrations());
        embedded::migrations::runner().run_async(&mut *backend).await?;
        Ok(())
    }
}
