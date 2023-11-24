use chrono::Duration;
use ring::rand::{SecureRandom, SystemRandom};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
#[error("Failed to generate token: {0}")]
pub struct TokenGeneratorError(String);

#[derive(Clone)]
pub struct TokenGenerator {
    ttl_access_token: Duration,
    ttl_single_access: Duration,
    random: SystemRandom,
}

impl TokenGenerator {
    pub fn new(ttl_access_token: Duration, ttl_single_access: Duration) -> Self {
        Self {
            ttl_access_token,
            ttl_single_access,
            random: SystemRandom::new(),
        }
    }

    pub fn ttl_access_token(&self) -> Duration {
        self.ttl_access_token
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
