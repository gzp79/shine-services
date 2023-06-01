use crate::db::{DBConfig, DBError};
use bb8::{Pool as BB8Pool};
use bb8_postgres::PostgresConnectionManager;
use tokio_postgres_rustls::MakeRustlsConnect;
use tokio_rustls::rustls;

type ConnectionPool = BB8Pool<PostgresConnectionManager<MakeRustlsConnect>>;

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
        Ok(Self { pool })        
    }
}
