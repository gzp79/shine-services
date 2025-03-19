mod query_builder;

pub use self::query_builder::*;
mod error_check;
pub use self::error_check::*;
mod pg_connection;
pub use self::pg_connection::*;
mod pg_type;
pub use self::pg_type::*;
mod pg_listener;
pub use self::pg_listener::*;

/// Create a prepared SQL statements
#[macro_export]
macro_rules! pg_prepared_statement {
    ($id:ident => $stmt:expr, [$($pid:ident:$pty:ty),*]) => {

        #[derive(Clone, Copy, Debug)]
        struct $id($crate::db::PGStatementId);

        impl $id {
            #[allow(dead_code)]
            pub async fn new(client: &$crate::db::PGClient) -> Result<Self, $crate::db::PGError>
            {
                log::debug!("Creating prepared statement for {}...", stringify!{$id});
                let params = vec![$(<$pty as $crate::db::ToPGType>::PG_TYPE,)*];
                log::trace!("Statement: {}\nArguments: {:#?}", $stmt, params);
                let stmt = client.create_prepared_statement($stmt, params).await;
                let _ = client.get_prepared_statement(stmt).await?;
                log::trace!("Creating prepared statement for {} done.", stringify!{$id});
                Ok(Self(stmt))
            }

            #[allow(dead_code)]
            pub async fn new_with_process<'a, F>(client: &$crate::db::PGClient, process: F) -> Result<Self, $crate::db::PGError>
            where
                F : FnOnce(&'a str) -> std::borrow::Cow<'a, str>
            {
                log::debug!("Creating prepared statement for {} with process...", stringify!{$id});
                let stmt = client.create_prepared_statement(&process($stmt), vec![$(<$pty as $crate::db::ToPGType>::PG_TYPE,)*]).await;
                let _ = client.get_prepared_statement(stmt).await?;
                log::trace!("Creating prepared statement for {} with process done.", stringify!{$id});
                Ok(Self(stmt))
            }

            #[allow(dead_code)]
            pub async fn statement<'a, T>(&self, client: &$crate::db::PGConnection<T>) -> Result<$crate::db::PGStatement, $crate::db::PGError>
            where
                T: $crate::db::PGRawConnection
            {
                client.get_prepared_statement(self.0).await
            }
        }
    }
}

/// Helper to create prepared SQL statements
#[macro_export]
macro_rules! pg_query {
    ($id:ident =>
        in = $($pid:ident: $pty:ty),*;
        out = $rid:ident: $rty:ty;
        sql = $stmt:expr ) => {

        $crate::pg_prepared_statement!($id => $stmt, [$($pid:$pty),*]);

        impl $id {
            #[allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            pub async fn query<'a, T>(
                &self,
                client: &$crate::db::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Vec<$rty>, $crate::db::PGError>
            where
                T: $crate::db::PGRawConnection
            {
                let statement = self.statement(client).await?;
                let rows = client.query(&statement, &[$($pid,)*]).await?;

                rows.into_iter().map(|row| row.try_get(&stringify!($rid))).collect::<Result<Vec<_>,_>>()
            }

            #[allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            pub async fn query_one<'a, T>(
                &self,
                client: &$crate::db::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<$rty, $crate::db::PGError>
            where
                T: $crate::db::PGRawConnection
            {
                let statement = self.statement(client).await?;
                let row = client.query_one(&statement, &[$($pid,)*]).await?;
                let value: $rty = row.try_get(&stringify!($rid))?;
                Ok(value)
            }

            #[allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            pub async fn query_opt<'a, T>(
                &self,
                client: &$crate::db::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Option<$rty>, $crate::db::PGError>
            where
                T: $crate::db::PGRawConnection
            {
                let statement = self.statement(client).await?;
                client.query_opt(&statement, &[$($pid,)*])
                    .await?
                    .map(|r| r.try_get(&stringify!($rid)))
                    .transpose()
            }
        }
    };

    ($id:ident =>
        in = $($pid:ident: $pty:ty),*;
        out = $oty:ty;
        sql = $stmt:expr ) => {

        $crate::pg_prepared_statement!($id => $stmt, [$($pid:$pty),*]);

        impl $id {
            #[allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            pub async fn query<'a, T>(
                &self,
                client: &$crate::db::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Vec<$oty>, $crate::db::PGError>
            where
                T: $crate::db::PGRawConnection
            {
                let statement = self.statement(client).await?;
                let rows = client.query(&statement, &[$($pid,)*]).await?;

                rows.into_iter()
                    .map(|row| <$oty as postgres_from_row::FromRow>::try_from_row(&row))
                    .collect::<Result<Vec<_>,_>>()
            }

            #[allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            pub async fn query_one<'a, T>(
                &self,
                client: &$crate::db::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<$oty, $crate::db::PGError>
            where
                T: $crate::db::PGRawConnection
            {
                let statement = self.statement(client).await?;
                let row = client
                    .query_one(&statement, &[$($pid,)*])
                    .await?;
                <$oty as postgres_from_row::FromRow>::try_from_row(&row)
            }

            #[allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            pub async fn query_opt<'a, T>(
                &self,
                client: &$crate::db::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Option<$oty>, $crate::db::PGError>
            where
                T: $crate::db::PGRawConnection
            {
                let statement = self.statement(client).await?;
                client.query_opt(&statement, &[$($pid,)*])
                    .await?
                    .map(|row| <$oty as postgres_from_row::FromRow>::try_from_row(&row) )
                    .transpose()
            }
        }
    };

    ($id:ident =>
        in = $($pid:ident: $pty:ty),*;
        sql = $stmt:expr ) => {

        $crate::pg_prepared_statement!($id => $stmt, [$($pid:$pty),*]);

        impl $id {
            #[allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            pub async fn execute<'a, T>(
                &self,
                client: &$crate::db::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<u64, $crate::db::PGError>
            where
                T: $crate::db::PGRawConnection
            {
                let statement = self.statement(client).await?;
                client.execute(&statement, &[$($pid,)*]).await
            }
        }
    };
}
