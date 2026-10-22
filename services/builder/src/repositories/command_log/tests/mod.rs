//! The command log on a real Postgres, through the `CommandLogDb` trait, with the chat room as the
//! replayed state: the happy paths and, mostly, the edge cases (concurrent writers, commands and
//! snapshots that do not belong on the log, damage the consistency check must find). They skip
//! without `SHINE_TEST_PG_CNS`.

mod pg_command_log_db;
mod test_utils;
