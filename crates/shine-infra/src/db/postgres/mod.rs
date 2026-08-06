mod error_check;
mod pg_connection;
mod pg_error;
mod pg_listener;
mod pg_type;
mod query_builder;

pub use self::{
    error_check::PgErrorChecks,
    pg_connection::{
        create_postgres_pool, PgClient, PgConfig, PgConnection, PgConnectionError, PgConnectionManager,
        PgConnectionPool, PgConvertError, PgPoolError, PgPooledConnection, PgRawClient, PgRawConnection, PgRawError,
        PgRawSocketConnection, PgRawTransaction, PgStatement, PgStatementId, PgTransaction, PgType, PostgresPoolStatus,
    },
    pg_error::PgError,
    pg_listener::{PgListener, PgNotification},
    pg_type::{
        PgValue, PgValueTypeBool, PgValueTypeInt2, PgValueTypeInt2Array, PgValueTypeInt4, PgValueTypeInt8,
        PgValueTypeTimestamptz, PgValueTypeUuid, PgValueTypeVarchar, PgValueTypeVarcharArray, ToPgType,
    },
    query_builder::{AndWhere, QueryBuilder},
};

/// Create a prepared SQL statements
#[macro_export]
macro_rules! pg_prepared_statement {
    ($id:ident => $stmt:expr, [$($pid:ident:$pty:ty),*]) => {

        #[derive(Clone, Copy, Debug)]
        struct $id($crate::db::postgres::PgStatementId);

        impl $id {
            #[allow(dead_code)]
            pub async fn new(client: &$crate::db::postgres::PgClient) -> Result<Self, $crate::db::postgres::PgRawError>
            {
                log::debug!("Creating prepared statement for {}...", stringify!{$id});
                let params = vec![$(<$pty as $crate::db::postgres::ToPgType>::PG_TYPE,)*];
                log::trace!("Statement: {}\nArguments: {:#?}", $stmt, params);
                let stmt = client.create_prepared_statement($stmt, params).await;
                let _ = client.get_prepared_statement(stmt).await?;
                log::trace!("Creating prepared statement for {} done.", stringify!{$id});
                Ok(Self(stmt))
            }

            #[allow(dead_code)]
            pub async fn new_with_process<'a, F>(client: &$crate::db::postgres::PgClient, process: F) -> Result<Self, $crate::db::postgres::PgRawError>
            where
                F : FnOnce(&'a str) -> std::borrow::Cow<'a, str>
            {
                log::debug!("Creating prepared statement for {} with process...", stringify!{$id});
                let stmt = client.create_prepared_statement(&process($stmt), vec![$(<$pty as $crate::db::postgres::ToPgType>::PG_TYPE,)*]).await;
                let _ = client.get_prepared_statement(stmt).await?;
                log::trace!("Creating prepared statement for {} with process done.", stringify!{$id});
                Ok(Self(stmt))
            }

            #[allow(dead_code)]
            pub async fn statement<T>(&self, client: &$crate::db::postgres::PgConnection<T>) -> Result<$crate::db::postgres::PgStatement, $crate::db::postgres::PgRawError>
            where
                T: $crate::db::postgres::PgRawConnection
            {
                client.get_prepared_statement(self.0).await
            }
        }
    }
}

/// Helper to create prepared SQL statements
#[macro_export]
macro_rules! pg_query {
    // The two row-returning shapes differ only in how a single row is turned into the output type;
    // this internal rule defines query/query_one/query_opt once, given `$row => $extract` as the
    // per-row conversion (a fragment, not a closure, so `$row`'s type is fixed at each use site).
    (@row_queries $id:ident, [$($pid:ident: $pty:ty),*], $rty:ty, $row:ident => $extract:expr) => {
        impl $id {
            #[allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            pub async fn query<T>(
                &self,
                client: &$crate::db::postgres::PgConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Vec<$rty>, $crate::db::postgres::PgRawError>
            where
                T: $crate::db::postgres::PgRawConnection
            {
                let statement = self.statement(client).await?;
                let rows = client.query(&statement, &[$($pid,)*]).await?;
                rows.into_iter().map(|$row| $extract).collect::<Result<Vec<_>,_>>()
            }

            #[allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            pub async fn query_one<T>(
                &self,
                client: &$crate::db::postgres::PgConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<$rty, $crate::db::postgres::PgRawError>
            where
                T: $crate::db::postgres::PgRawConnection
            {
                let statement = self.statement(client).await?;
                let $row = client.query_one(&statement, &[$($pid,)*]).await?;
                $extract
            }

            #[allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            pub async fn query_opt<T>(
                &self,
                client: &$crate::db::postgres::PgConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Option<$rty>, $crate::db::postgres::PgRawError>
            where
                T: $crate::db::postgres::PgRawConnection
            {
                let statement = self.statement(client).await?;
                client.query_opt(&statement, &[$($pid,)*])
                    .await?
                    .map(|$row| $extract)
                    .transpose()
            }
        }
    };

    ($id:ident =>
        in = $($pid:ident: $pty:ty),*;
        out = $rid:ident: $rty:ty;
        sql = $stmt:expr ) => {

        $crate::pg_prepared_statement!($id => $stmt, [$($pid:$pty),*]);
        $crate::pg_query!(@row_queries $id, [$($pid: $pty),*], $rty,
            row => row.try_get(&stringify!($rid)));
    };

    ($id:ident =>
        in = $($pid:ident: $pty:ty),*;
        out = $oty:ty;
        sql = $stmt:expr ) => {

        $crate::pg_prepared_statement!($id => $stmt, [$($pid:$pty),*]);
        $crate::pg_query!(@row_queries $id, [$($pid: $pty),*], $oty,
            row => <$oty as postgres_from_row::FromRow>::try_from_row(&row));
    };

    ($id:ident =>
        in = $($pid:ident: $pty:ty),*;
        sql = $stmt:expr ) => {

        $crate::pg_prepared_statement!($id => $stmt, [$($pid:$pty),*]);

        impl $id {
            #[allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            pub async fn execute<T>(
                &self,
                client: &$crate::db::postgres::PgConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<u64, $crate::db::postgres::PgRawError>
            where
                T: $crate::db::postgres::PgRawConnection
            {
                let statement = self.statement(client).await?;
                client.execute(&statement, &[$($pid,)*]).await
            }
        }
    };
}
