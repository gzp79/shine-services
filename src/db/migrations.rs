use futures::future::BoxFuture;
use sqlx::{
    error::BoxDynError as SqlxBoxError,
    migrate::{Migration, MigrationSource, MigrationType, Migrator},
    AnyPool,
};
use sqlx_interpolation::{sql, types, DBKind};

use crate::app_error::AppError;

#[derive(Debug)]
pub struct Migrations;

impl Migrations {
    pub async fn apply(self, pool: &AnyPool) -> Result<(), AppError> {
        let kind = DBKind::from(pool.any_kind());
        let migrator = Migrator::new(AnyMigrations(Self, kind)).await?;
        migrator.run(pool).await?;
        Ok(())
    }
}

#[derive(Debug)]
struct AnyMigrations(Migrations, DBKind);

impl<'s> MigrationSource<'s> for AnyMigrations {
    #[allow(clippy::type_complexity)]
    fn resolve(self) -> BoxFuture<'s, Result<Vec<Migration>, SqlxBoxError>> {
        Box::pin(async move {
            let migrations: Vec<Migration> = vec![
                MigrateCreateIdentity(0, self.1).try_into()?,
                //MigrateExternalProviders(1, self.1).try_into()?,
            ];
            Ok(migrations)
        })
    }
}

struct MigrateCreateIdentity(i64, DBKind);

impl TryFrom<MigrateCreateIdentity> for Migration {
    type Error = AppError;

    fn try_from(m: MigrateCreateIdentity) -> Result<Migration, AppError> {
        let mut query = m.1.query_builder();

        sql!(
            query,
            "CREATE TABLE identities"
                + "("
                + "  user_id UUID NOT NULL PRIMARY KEY,"
                + "  kind INTEGER NOT NULL,"
                + "  created TIMESTAMPTZ NULL"
                + ");"
        );

        let query = query.into_raw()?;

        Ok(Migration::new(
            m.0,
            "Create identities table".into(),
            MigrationType::Simple,
            query.into(),
        ))
    }
}
