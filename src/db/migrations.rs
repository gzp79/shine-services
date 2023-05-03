use crate::db::DBError;
use futures::future::BoxFuture;
use sqlx::{
    error::BoxDynError as SqlxBoxError,
    migrate::{Migration, MigrationSource, MigrationType, Migrator},
    AnyPool,
};
use sqlx_interpolation::{sql, DBKind};

#[derive(Debug)]
pub struct Migrations;

impl Migrations {
    pub async fn apply(self, pool: &AnyPool) -> Result<(), DBError> {
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
                MigrateCreateExternalLogin(1, self.1).try_into()?,
            ];
            Ok(migrations)
        })
    }
}

struct MigrateCreateIdentity(i64, DBKind);

impl TryFrom<MigrateCreateIdentity> for Migration {
    type Error = DBError;

    fn try_from(m: MigrateCreateIdentity) -> Result<Migration, DBError> {
        let mut query = m.1.query_builder();

        sql!(
            query,
            "CREATE TABLE identities"
                + "("
                + "  user_id UUID NOT NULL PRIMARY KEY,"
                + "  kind INTEGER NOT NULL,"
                + "  created TIMESTAMPTZ NULL,"
                + "  name VARCHAR(64) NOT NULL,"
                + "  email VARCHAR(256),"
                + "  profile_image TEXT"
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

struct MigrateCreateExternalLogin(i64, DBKind);

impl TryFrom<MigrateCreateExternalLogin> for Migration {
    type Error = DBError;

    fn try_from(m: MigrateCreateExternalLogin) -> Result<Migration, DBError> {
        let mut query = m.1.query_builder();

        sql!(
            query,
            "CREATE TABLE external_logins"
                + "("
                + "  user_id UUID NOT NULL PRIMARY KEY,"
                + "  provider TEXT NOT NULL,"
                + "  provider_id TEXT NOT NULL,"
                + "  linked TIMESTAMPTZ NULL,"
                + "  CONSTRAINT user_id_fkey FOREIGN KEY(user_id) REFERENCES identities(user_id)"
                + ");"
                + "CREATE UNIQUE INDEX externalLogins_provider_provider_id ON external_logins(provider, provider_id);"
        );

        let query = query.into_raw()?;

        Ok(Migration::new(
            m.0,
            "Create external login tables".into(),
            MigrationType::Simple,
            query.into(),
        ))
    }
}
