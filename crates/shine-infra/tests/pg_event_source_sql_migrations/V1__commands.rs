use shine_infra::db::event_source::pg::migration_001;

pub fn migration() -> String {
    migration_001("test")
}
