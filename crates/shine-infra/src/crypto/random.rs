use ring::rand::{SecureRandom, SystemRandom};

/// Generate a random hex string of 16 bytes
pub fn hex_16(random: &SystemRandom) -> String {
    let mut raw = [0u8; 16];
    random.fill(&mut raw).unwrap();
    hex::encode(raw)
}
