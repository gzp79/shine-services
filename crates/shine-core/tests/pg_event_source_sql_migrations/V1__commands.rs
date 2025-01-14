use shine_core::db::event_source::pg::migration_001;

pub fn migration() -> String {
    migration_001("test")
}
