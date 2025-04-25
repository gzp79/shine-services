use crate::db::cacerts::{get_root_cert_store, CertError};
use crate::db::DBError;
use bb8::{ManageConnection, Pool as BB8Pool, PooledConnection, RunError};
use bb8_postgres::PostgresConnectionManager;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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
        {
            let prepared_statements = self.prepared_statements.read().await;
            if let Some(prepared_statements) = prepared_statements.get(&prepared_id.0) {
                return Ok(prepared_statements.to_owned());
            }
        }

        let prepared_statements_builder = self.prepared_statements_builder.read().await;
        if let Some((stmt, types)) = prepared_statements_builder.get(&prepared_id.0) {
            // create a new prepared statement for the current connection
            let mut prepared_statements = self.prepared_statements.write().await;
            let prepared = self.client.prepare_typed(stmt, types).await?;
            prepared_statements.insert(prepared_id.0, prepared.clone());
            Ok(prepared)
        } else {
            //todo: return some PGError instead of panic
            panic!("No prepared statement found for id: {}", prepared_id.0);
        }
    }

    #[inline]
    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), DBError>
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.listener.listen(channel, handler).await
    }

    #[inline]
    pub async fn unlisten(&self, channel: &str) -> Result<(), DBError> {
        self.listener.unlisten(channel).await
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
    pub fn new(config: PGConfig, tls: MakeRustlsConnect) -> Self {
        let connection_manager = PostgresConnectionManager::new(config.clone(), tls.clone());
        let listener = PGListener::new(config, tls);

        Self {
            connection_manager,
            prepared_statement_id: Arc::new(AtomicUsize::new(1)),
            prepared_statements_builder: Arc::new(RwLock::new(HashMap::default())),
            listener,
        }
    }
}

impl Drop for PGConnectionManager {
    fn drop(&mut self) {
        self.listener.close();
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
        conn.simple_query("").await.map(|_| ())
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
}

pub async fn create_postgres_pool(cns: &str) -> Result<PGConnectionPool, PGCreatePoolError> {
    let certs = get_root_cert_store().map_err(PGCreatePoolError::CertError)?;
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(certs)
        .with_no_client_auth();

    let tls = MakeRustlsConnect::new(tls_config);

    let pg_config = PGConfig::from_str(cns)?;
    //log::debug!("Postgresql config: {pg_config:#?}");
    let postgres_manager = PGConnectionManager::new(pg_config, tls);
    let postgres = bb8::Pool::builder()
        .max_size(10) // Set the maximum number of connections in the pool
        .build(postgres_manager)
        .await?;

    Ok(postgres)
}
