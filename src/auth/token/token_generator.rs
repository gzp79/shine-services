use chrono::Duration;
use ring::rand::{SecureRandom, SystemRandom};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
#[error("Failed to generate token: {0}")]
pub struct TokenGeneratorError(String);

#[derive(Clone)]
pub struct TokenGenerator {
    ttl_remember_me: Duration,
    ttl_single_access: Duration,
    random: SystemRandom,
}

impl TokenGenerator {
    pub fn new(ttl_remember_me: Duration, ttl_single_access: Duration) -> Self {
        Self {
            ttl_remember_me,
            ttl_single_access,
            random: SystemRandom::new(),
        }
    }

    pub fn ttl_remember_me(&self) -> Duration {
        self.ttl_remember_me
    }

    pub fn ttl_single_access(&self) -> Duration {
        self.ttl_single_access
    }

    pub fn generate_token(&self) -> Result<String, TokenGeneratorError> {
        let mut raw = [0_u8; 16];
        self.random
            .fill(&mut raw)
            .map_err(|err| TokenGeneratorError(format!("{err:#?}")))?;
        Ok(hex::encode(raw))
    }
}
