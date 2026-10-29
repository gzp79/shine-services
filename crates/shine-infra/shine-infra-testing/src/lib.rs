pub mod db {
    pub mod postgres {
        pub use shine_infra::db::postgres::testing::*;
    }
    pub mod redis {
        pub use shine_infra::db::redis::testing::*;
    }
}
