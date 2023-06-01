mod db_config;
pub use self::db_config::*;
mod db_error;
pub use self::db_error::*;
mod db_pool;
pub use self::db_pool::*;

mod identity_manager;
pub use self::identity_manager::*;
mod session_manager;
pub use self::session_manager::*;

pub trait DBErrorChecks {
    fn is_constraint(&self, table: &str, constraint: &str) -> bool;
}

impl DBErrorChecks for tokio_postgres::Error {
    fn is_constraint(&self, table: &str, constraint: &str) -> bool {
        if let Some(err) = err.as_db_error() {
            if &SqlState::UNIQUE_VIOLATION == err.code() {
                if err.table() == Some("identities") && err.message().contains("idx_name") {
                    return true;
                }
            }
        }
        false
    }
}
