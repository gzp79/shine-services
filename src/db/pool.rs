use crate::db::{migrations::Migrations, DBError};
use sqlx::{any::AnyPoolOptions, migrate::MigrateDatabase, AnyPool};

pub async fn create_pool(cns: &str) -> Result<AnyPool, DBError> {
    if !sqlx::Any::database_exists(cns).await? {
        sqlx::Any::create_database(cns).await?;
    }

    let pool = AnyPoolOptions::new().max_connections(5).connect(cns).await?;
    Migrations.apply(&pool).await?;
    Ok(pool)
}
