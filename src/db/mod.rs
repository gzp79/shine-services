mod db_config;
pub use self::db_config::*;
mod db_error;
pub use self::db_error::*;
mod db_pool;
pub use self::db_pool::*;

mod identity_manager;
pub use self::identity_manager::*;
mod session_key;
pub use self::session_key::*;
mod session_manager;
pub use self::session_manager::*;

mod query_builder;
pub use self::query_builder::*;

/// A shorthand used for the return types in the ToSql and FromSql implementations.
pub type PGError = Box<dyn std::error::Error + Sync + Send>;

/// Helper to create prepared SQL statements
#[macro_export]
macro_rules! pg_prepared_statement {
    ($id:ident => $stmt:expr, [$($ty:ident),*]) => {
        struct $id(tokio_postgres::Statement);

        impl $id {
            async fn new(client: &bb8::PooledConnection<'_, $crate::db::PGConnection>) -> Result<Self, $crate::db::DBError> {
                let stmt = client
                    .prepare_typed($stmt, &[$(tokio_postgres::types::Type::$ty,)*])
                    .await
                    .map_err(DBError::from)?;
                Ok(Self(stmt))
            }
        }

        impl std::ops::Deref for $id {
            type Target = tokio_postgres::Statement;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}
