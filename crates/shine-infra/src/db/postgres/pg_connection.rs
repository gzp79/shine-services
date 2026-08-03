use crate::db::cacerts::{get_root_cert_store, CertError};
use crate::db::DBError;
use crate::health::StatusProvider;
use async_trait::async_trait;
use bb8::{ManageConnection, Pool as BB8Pool, PooledConnection, RunError};
use bb8_postgres::PostgresConnectionManager;
use refinery::{Migration, Runner};
use std::ops::Deref;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, ops::DerefMut};
use thiserror::Error as ThisError;
use tokio::sync::RwLock;
use tokio_postgres::{tls::MakeTlsConnect, GenericClient, IsolationLevel, Statement};
use tokio_postgres_rustls::MakeRustlsConnect;

use super::PGListener;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PGStatementId(usize);

pub trait PGRawConnection: GenericClient {}
impl<T> PGRawConnection for T where T: GenericClient {}

type PreparedStatementBuilder = (String, Vec<PGType>);

pub struct PGConnection<T>
where
    T: PGRawConnection,
{
    prepared_statement_id: Arc<AtomicUsize>,
    prepared_statements_builder: Arc<RwLock<HashMap<usize, PreparedStatementBuilder>>>,
    prepared_statements: Arc<RwLock<HashMap<usize, Statement>>>,
    client: T,
    listener: PGListener,
}

impl<T: PGRawConnection> PGConnection<T> {
    #[inline]
    pub async fn create_prepared_statement(&self, stmt: &str, types: Vec<PGType>) -> PGStatementId {
        let id = self.prepared_statement_id.fetch_add(1, Ordering::Relaxed);
        let mut prepared_statements = self.prepared_statements_builder.write().await;
        prepared_statements.insert(id, (stmt.to_string(), types));
        PGStatementId(id)
    }

    #[inline]
    pub async fn get_prepared_statement(&self, prepared_id: PGStatementId) -> Result<Statement, PGError> {
        // Fast path: already prepared on this connection.
        if let Some(prepared) = self.prepared_statements.read().await.get(&prepared_id.0) {
            return Ok(prepared.to_owned());
        }

        // Copy the builder entry out so the prepare round-trip below holds no lock. Ids come only
        // from create_prepared_statement, never from user/DB input, so a missing id is a program
        // error, not a recoverable condition.
        let Some((stmt, types)) = self
            .prepared_statements_builder
            .read()
            .await
            .get(&prepared_id.0)
            .cloned()
        else {
            panic!(
                "No prepared statement registered for id {}: statement was not built, or a handle was used across pools (independent id spaces)",
                prepared_id.0
            );
        };

        let prepared = self.client.prepare_typed(&stmt, &types).await?;

        // A pooled connection is used by one task at a time, so a concurrent prepare of the same id
        // cannot happen; entry() keeps the cache single-valued if that assumption ever changes
        // (the redundant prepared statement is dropped, deallocating it server-side).
        Ok(self
            .prepared_statements
            .write()
            .await
            .entry(prepared_id.0)
            .or_insert(prepared)
            .clone())
    }

    #[inline]
    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), DBError>
    where
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        self.listener.listen(channel, handler).await
    }

    #[inline]
    pub async fn unlisten(&self, channel: &str) -> Result<(), DBError> {
        self.listener.unlisten(channel).await
    }

    #[inline]
    pub async fn listener_backend_pid(&self) -> Option<i32> {
        self.listener.backend_pid().await
    }
}

impl PGConnection<PGRawClient> {
    fn new(
        pg_client: PGRawClient,
        listener: PGListener,
        prepared_statement_id: Arc<AtomicUsize>,
        prepared_statements_builder: Arc<RwLock<HashMap<usize, PreparedStatementBuilder>>>,
    ) -> Self {
        Self {
            prepared_statement_id,
            prepared_statements_builder,
            prepared_statements: Arc::new(RwLock::new(HashMap::default())),
            client: pg_client,
            listener,
        }
    }

    /// Handle migration manually. Allows to keep multiple (independent) migration in a single
    /// database.
    pub async fn migrate(&mut self, name: &str, migrations: &[String]) -> Result<(), DBError> {
        let migrations = migrations
            .iter()
            .inspect(|m| log::debug!("Migration: {m}"))
            .enumerate()
            .map(|(i, m)| Migration::unapplied(&format!("V{i}__{name}"), m))
            .collect::<Result<Vec<_>, _>>()?;

        let mut runner = Runner::new(&migrations);
        runner.set_migration_table_name(format!("__migration__{name}"));

        runner
            .run_async(&mut self.client)
            .await
            .map_err(DBError::SqlMigration)?;
        Ok(())
    }

    #[inline]
    pub async fn transaction(
        &mut self,
        isolation_level: Option<IsolationLevel>,
    ) -> Result<PGConnection<PGRawTransaction<'_>>, PGError> {
        let mut transaction_builder = self.client.build_transaction();
        if let Some(level) = isolation_level {
            transaction_builder = transaction_builder.isolation_level(level);
        }
        let transaction = transaction_builder.start().await?;
        Ok(PGConnection {
            prepared_statement_id: self.prepared_statement_id.clone(),
            prepared_statements_builder: self.prepared_statements_builder.clone(),
            prepared_statements: self.prepared_statements.clone(),
            client: transaction,
            listener: self.listener.clone(),
        })
    }
}

impl PGConnection<PGRawTransaction<'_>> {
    pub async fn commit(self) -> Result<(), PGError> {
        self.client.commit().await
    }

    pub async fn rollback(self) -> Result<(), PGError> {
        self.client.rollback().await
    }
}

impl<T: PGRawConnection> Deref for PGConnection<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl<T: PGRawConnection> DerefMut for PGConnection<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

pub struct PGConnectionManager {
    connection_manager: PostgresConnectionManager<MakeRustlsConnect>,
    prepared_statement_id: Arc<AtomicUsize>,
    prepared_statements_builder: Arc<RwLock<HashMap<usize, PreparedStatementBuilder>>>,
    listener: PGListener,
}

impl PGConnectionManager {
    pub fn new(config: PGConfig, tls: MakeRustlsConnect, max_reconnect_backoff: Duration) -> Self {
        let connection_manager = PostgresConnectionManager::new(config.clone(), tls.clone());
        let listener = PGListener::new(config, tls, max_reconnect_backoff);

        Self {
            connection_manager,
            prepared_statement_id: Arc::new(AtomicUsize::new(1)),
            prepared_statements_builder: Arc::new(RwLock::new(HashMap::default())),
            listener,
        }
    }
}

impl bb8::ManageConnection for PGConnectionManager {
    type Connection = PGConnection<PGRawClient>;
    type Error = PGError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let conn = self.connection_manager.connect().await?;
        Ok(PGConnection::new(
            conn,
            self.listener.clone(),
            self.prepared_statement_id.clone(),
            self.prepared_statements_builder.clone(),
        ))
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        self.connection_manager.is_valid(&mut conn.client).await
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.connection_manager.has_broken(&mut conn.client)
    }
}

pub type PGConnectionError = RunError<<PGConnectionManager as ManageConnection>::Error>;
pub type PGConnectionPool = BB8Pool<PGConnectionManager>;
pub type PGPooledConnection<'a> = PooledConnection<'a, PGConnectionManager>;
pub type PGError = tokio_postgres::Error;
pub type PGStatement = tokio_postgres::Statement;

pub type PGConfig = tokio_postgres::Config;
pub type PGType = tokio_postgres::types::Type;

pub type PGRawClient = tokio_postgres::Client;
type PGSocket = tokio_postgres::Socket;
type PGSocketStream = <MakeRustlsConnect as MakeTlsConnect<PGSocket>>::Stream;
pub type PGRawSocketConnection = tokio_postgres::Connection<PGSocket, PGSocketStream>;
pub type PGRawTransaction<'a> = tokio_postgres::Transaction<'a>;
pub type PGClient = PGConnection<PGRawClient>;
pub type PGTransaction<'a> = PGConnection<PGRawTransaction<'a>>;

/// A shorthand used for the return types in the ToSql and FromSql implementations.
pub type PGConvertError = Box<dyn std::error::Error + Sync + Send>;

#[derive(ThisError, Debug)]
pub enum PGCreatePoolError {
    #[error(transparent)]
    PgError(#[from] PGError),
    #[error("Certificate load error")]
    CertError(#[source] CertError),
    #[error(transparent)]
    ConfigError(#[from] crate::db::CnsParamError),
    #[error("Connection string parameter {0:?} must be greater than zero")]
    InvalidPoolParam(&'static str),
}

pub struct PostgresPoolStatus {
    pool: PGConnectionPool,
}

impl PostgresPoolStatus {
    pub fn new(pool: PGConnectionPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StatusProvider for PostgresPoolStatus {
    fn name(&self) -> &'static str {
        "postgres"
    }

    async fn status(&self) -> serde_json::Value {
        let state = self.pool.state();
        serde_json::json!({
            "connections": state.connections,
            "idleConnections": state.idle_connections
        })
    }
}

/// Format: `postgres://...?connect_timeout=3&pool_timeout=5&max_size=10`
/// - `connect_timeout`: PostgreSQL native parameter in SECONDS (TCP connection establishment)
/// - `pool_timeout`: custom parameter in SECONDS for bb8 pool (acquiring connection from pool, including waiting for connection to be established if pool is exhausted)
/// - `max_size`: custom parameter for the maximum number of pooled connections (default 10)
pub async fn create_postgres_pool(cns: &str) -> Result<PGConnectionPool, PGCreatePoolError> {
    // Parse and validate before the TLS/cert setup so a bad connection string fails fast without
    // touching the cert store or a crypto provider.
    let mut cns = crate::db::ConnectionString::parse(cns);
    let pool_timeout_s = cns.take_u64("pool_timeout")?.unwrap_or(30);
    let max_size = cns.take_u64("max_size")?.unwrap_or(10);
    let cns_clean = cns.into_cns();

    // Reject the degenerate values bb8 would otherwise accept: max_size=0 builds a pool that can
    // never hand out a connection (and a value > u32::MAX would truncate to 0 in the cast below),
    // and pool_timeout=0 makes every checkout time out immediately.
    if max_size == 0 || max_size > u32::MAX as u64 {
        return Err(PGCreatePoolError::InvalidPoolParam("max_size"));
    }
    if pool_timeout_s == 0 {
        return Err(PGCreatePoolError::InvalidPoolParam("pool_timeout"));
    }
    let pool_timeout = std::time::Duration::from_secs(pool_timeout_s);
    let max_size = max_size as u32;

    let certs = get_root_cert_store().map_err(PGCreatePoolError::CertError)?;
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(certs)
        .with_no_client_auth();

    let tls = MakeRustlsConnect::new(tls_config);

    let pg_config = PGConfig::from_str(&cns_clean)?;
    let postgres_manager = PGConnectionManager::new(pg_config, tls, pool_timeout);
    let postgres = bb8::Pool::builder()
        .max_size(max_size)
        .connection_timeout(pool_timeout)
        .build(postgres_manager)
        .await?;

    Ok(postgres)
}

#[cfg(test)]
mod test {
    use super::*;

    // Degenerate pool params are rejected before any network I/O, so these run without a server.
    // Parsing (non-integer / duplicate values) is covered by the connection-string parser's own
    // tests; here only the range checks are exercised.
    fn invalid_param(err: PGCreatePoolError) -> &'static str {
        match err {
            PGCreatePoolError::InvalidPoolParam(name) => name,
            other => panic!("expected InvalidPoolParam, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn max_size_zero_is_rejected_not_panicked() {
        let err = create_postgres_pool("postgres://localhost?max_size=0")
            .await
            .unwrap_err();
        assert_eq!(invalid_param(err), "max_size");
    }

    #[tokio::test]
    async fn max_size_overflowing_u32_is_rejected() {
        let cns = format!("postgres://localhost?max_size={}", u32::MAX as u64 + 1);
        let err = create_postgres_pool(&cns).await.unwrap_err();
        assert_eq!(invalid_param(err), "max_size");
    }

    #[tokio::test]
    async fn pool_timeout_zero_is_rejected() {
        let err = create_postgres_pool("postgres://localhost?pool_timeout=0")
            .await
            .unwrap_err();
        assert_eq!(invalid_param(err), "pool_timeout");
    }
}
