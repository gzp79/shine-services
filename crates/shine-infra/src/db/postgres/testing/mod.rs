#![cfg(any(test, feature = "testing"))]

mod utils;

pub use self::utils::get_postgres_cns;
