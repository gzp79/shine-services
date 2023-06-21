use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
#[error("Failed to generate name")]
pub struct NameGeneratorError;

#[derive(Clone)]
pub struct NameGenerator {}

impl NameGenerator {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn generate_name(&self) -> Result<String, NameGeneratorError> {
        let id = Uuid::new_v4();
        Ok(id.hyphenated().to_string())
    }
}
