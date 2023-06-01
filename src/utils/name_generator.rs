use uuid::Uuid;

pub fn generate_name() -> String {
    let id = Uuid::new_v4();
    id.hyphenated().to_string()
}
