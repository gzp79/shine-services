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
mod name_generator;
pub use self::name_generator::*;

/// A shorthand used for the return types in the ToSql and FromSql implementations.
pub type PGError = Box<dyn std::error::Error + Sync + Send>;
