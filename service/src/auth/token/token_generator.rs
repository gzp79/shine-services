use ring::rand::{SecureRandom, SystemRandom};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
#[error("Failed to generate token: {0}")]
pub struct TokenGeneratorError(String);

#[derive(Clone)]
pub struct TokenGenerator<'a> {
    random: &'a SystemRandom,
}

impl<'a> TokenGenerator<'a> {
    pub fn new(random: &'a SystemRandom) -> Self {
        Self { random }
    }

    pub fn generate_token(&self) -> Result<String, TokenGeneratorError> {
        let mut raw = [0_u8; 16];
        self.random
            .fill(&mut raw)
            .map_err(|err| TokenGeneratorError(format!("{err:#?}")))?;
        Ok(hex::encode(raw))
    }
}
