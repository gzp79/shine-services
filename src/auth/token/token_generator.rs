use chrono::Duration;
use ring::rand::{SecureRandom, SystemRandom};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
#[error("Failed to generate token: {0}")]
pub(in crate::auth) struct TokenGeneratorError(String);

#[derive(Clone)]
pub(in crate::auth) struct TokenGenerator {
    token_max_duration: Duration,
    random: SystemRandom,
}

impl TokenGenerator {
    pub fn new(token_max_duration: Duration) -> Self {
        Self {
            token_max_duration,
            random: SystemRandom::new(),
        }
    }

    pub fn max_duration(&self) -> Duration {
        self.token_max_duration
    }

    pub fn generate_token(&self) -> Result<String, TokenGeneratorError> {
        let mut raw = [0_u8; 16];
        self.random
            .fill(&mut raw)
            .map_err(|err| TokenGeneratorError(format!("{err:#?}")))?;
        Ok(hex::encode(raw))
    }
}
