use crate::db::{DBPool, DBError};

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./sql_migrations");
}

struct DBMigration;

impl DBMigration {
    pub async fn migrate(pool: &DBPool) -> Result<(), DBError> {
        todo!()
    }
}