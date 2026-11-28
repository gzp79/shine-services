-- Purge scripts for the builder and identity databases.
--
-- DESTRUCTIVE: every statement drops data. Check which database you are connected to first.
-- Run ONE section per database: builder and identity each use their own Postgres database.
--
-- Built from the CREATE statements in services/*/sql_migrations (tables, indexes, sequence,
-- function, trigger, type). Update this file when a migration adds another kind of object.
--
-- refinery_schema_history must be dropped too. If it stays, refinery treats V1 as already applied
-- and aborts start-up with a "divergent" error when a migration file changed.


-- ============================================================================
-- Builder database (services/builder/sql_migrations)
-- ============================================================================

DROP TABLE IF EXISTS cl_snapshots CASCADE;
DROP TABLE IF EXISTS cl_commands CASCADE;   -- also drops trigger cl_commands_no_update
DROP TABLE IF EXISTS cl_streams CASCADE;
DROP FUNCTION IF EXISTS cl_commands_immutable() CASCADE;
DROP TYPE IF EXISTS cl_command_input CASCADE;

DROP TABLE IF EXISTS refinery_schema_history;


-- ============================================================================
-- Identity database (services/identity/sql_migrations)
-- ============================================================================

DROP TABLE IF EXISTS login_tokens CASCADE;
DROP TABLE IF EXISTS roles CASCADE;
DROP TABLE IF EXISTS external_logins CASCADE;
DROP TABLE IF EXISTS identities CASCADE;
DROP SEQUENCE IF EXISTS user_id_counter;

DROP TABLE IF EXISTS refinery_schema_history;


-- ============================================================================
-- Alternative: wipe the whole public schema (does not depend on the lists above)
-- ============================================================================
-- This also resets the schema's ownership and grants. On a managed server you may have to
-- re-grant privileges to the application role afterwards.
--
-- DROP SCHEMA public CASCADE;
-- CREATE SCHEMA public;
