use serde::{Deserialize, Serialize};
use shine_core::db::{self, DBError, PGConnectionPool};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DBConfig {
    pub sql_cns: String,
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./sql_migrations");
}

#[derive(Clone)]
pub struct DBPool {
    pub postgres: PGConnectionPool,
}

impl DBPool {
    pub async fn new(config: &DBConfig) -> Result<Self, DBError> {
        log::info!("Creating postgres pool...");
        let postgres = db::create_postgres_pool(config.sql_cns.as_str())
            .await
            .map_err(DBError::PGCreatePoolError)?;

        let pool = Self { postgres };
        pool.migrate().await?;
        Ok(pool)
    }

    async fn migrate(&self) -> Result<(), DBError> {
        log::info!("Migrating database...");
        let mut backend = self.postgres.get().await.map_err(DBError::PGPoolError)?;
        log::debug!("migrations: {:#?}", embedded::migrations::runner().get_migrations());
        let client = &mut **backend;
        embedded::migrations::runner().run_async(client).await?;
        Ok(())
    }
}
