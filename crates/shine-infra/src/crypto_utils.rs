use ring::{aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM}, hmac::{self, Key}};
use rand::{rngs::OsRng, TryRngCore};
use thiserror::Error as ThisError;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

const NONCE_LEN: usize = 12;

#[derive(Debug, ThisError)]
pub enum CryptoError {
    #[error("Failed to encrypt data")]
    EncryptionError,
    #[error("Failed to decrypt data")]
    DecryptionError,
    #[error("Invalid key length")]
    InvalidKeyLength,
}

pub struct CryptoUtils {
    encryption_key: LessSafeKey,
    hmac_key: Key,
}

impl CryptoUtils {
    pub fn new(encryption_key: &[u8], hmac_key: &[u8]) -> Result<Self, CryptoError> {
        let encryption_key = UnboundKey::new(&AES_256_GCM, encryption_key)
            .map_err(|_| CryptoError::InvalidKeyLength)?;
        let encryption_key = LessSafeKey::new(encryption_key);
        let hmac_key = Key::new(hmac::HMAC_SHA256, hmac_key);
        Ok(Self { encryption_key, hmac_key })
    }

    pub fn encrypt(&self, data: &str) -> Result<String, CryptoError> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.try_fill_bytes(&mut nonce_bytes).map_err(|_| CryptoError::EncryptionError)?;
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);

        let mut in_out = data.as_bytes().to_vec();
        self.encryption_key
            .seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
            .map_err(|_| CryptoError::EncryptionError)?;

        let mut result = Vec::with_capacity(NONCE_LEN + in_out.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&in_out);

        Ok(URL_SAFE_NO_PAD.encode(result))
    }

    pub fn decrypt(&self, data: &str) -> Result<String, CryptoError> {
        let decoded = URL_SAFE_NO_PAD.decode(data).map_err(|_| CryptoError::DecryptionError)?;
        if decoded.len() < NONCE_LEN {
            return Err(CryptoError::DecryptionError);
        }

        let (nonce_bytes, ciphertext) = decoded.split_at(NONCE_LEN);
        let nonce = Nonce::assume_unique_for_key(nonce_bytes.try_into().unwrap());

        let mut in_out = ciphertext.to_vec();
        let decrypted_data = self.encryption_key
            .open_in_place(nonce, Aad::empty(), &mut in_out)
            .map_err(|_| CryptoError::DecryptionError)?;
        String::from_utf8(decrypted_data.to_vec()).map_err(|_| CryptoError::DecryptionError)
    }

    pub fn hash(&self, data: &str) -> String {
        let signature = hmac::sign(&self.hmac_key, data.as_bytes());
        URL_SAFE_NO_PAD.encode(signature.as_ref())
    }
}

pub fn generate_key() -> Result<String, CryptoError> {
    let mut key = [0u8; 32];
    OsRng.try_fill_bytes(&mut key).map_err(|_| CryptoError::EncryptionError)?;
    let encoded_key = URL_SAFE_NO_PAD.encode(&key);
    Ok(encoded_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn generate_new_keys() {
        let encryption_key = generate_key().unwrap();
        let hmac_key = generate_key().unwrap();
        println!("Encryption Key: {}", encryption_key);
        println!("HMAC Key: {}", hmac_key);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let encryption_key = generate_key().unwrap();
        let hmac_key = generate_key().unwrap();
        let crypto = CryptoUtils::new(&URL_SAFE_NO_PAD.decode(encryption_key).unwrap(), &URL_SAFE_NO_PAD.decode(hmac_key).unwrap()).unwrap();
        let data = "hello world";
        let encrypted = crypto.encrypt(data).unwrap();
        let decrypted = crypto.decrypt(&encrypted).unwrap();
        assert_eq!(data, decrypted);
    }
}
