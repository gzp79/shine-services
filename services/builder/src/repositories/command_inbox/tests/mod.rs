//! The command inbox on a real Redis, through the `CommandInboxDb` trait: ordering and paging of the
//! polls, entries the reader cannot use, trimming, expiry and the wake notifications. They skip
//! without `SHINE_TEST_REDIS_CNS`.

mod redis_command_inbox_db;
mod test_utils;
