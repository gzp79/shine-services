use chrono::Duration;
use ring::rand::SystemRandom;

pub(in crate::auth) struct TokenClient {
    pub token_max_duration: Duration,
    pub random: SystemRandom,
}

impl TokenClient {
    pub fn new(token_max_duration: Duration) -> Self {
        Self {
            token_max_duration,
            random: SystemRandom::new(),
        }
    }
}
