use hex;
use ring::digest;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, ops::Deref};
use utoipa::ToSchema;
use validator::{ValidateEmail, ValidationError};

/// Normalize an email address for consistent storage and comparison.
pub fn normalize_email(email: &str) -> String {
    email.to_lowercase()
}

/// Generate a (crypto) hashed version of an email.
pub fn hash_email(email_address: &str) -> String {
    debug_assert_eq!(email_address, normalize_email(email_address));
    let hash = digest::digest(&digest::SHA256, email_address.as_bytes());
    let hash = hex::encode(hash);
    log::debug!("Hashing email: {email_address:?} -> [{hash}]");
    hash
}

/// Validated and normalized email address.
/// - Email is validated (proper format)
/// - Email is normalized (lowercase for case-insensitive comparison)
///
/// Use this type instead of `String` for email addresses to enforce invariants at compile time.
#[derive(Clone, Debug, PartialEq, Eq, Hash, ToSchema)]
#[schema(value_type = String, format = "email", example = "user@example.com")]
pub struct Email(String);

impl Email {
    /// Create a new Email from a string, validating and normalizing it.
    pub fn new(email: impl AsRef<str>) -> Result<Self, ValidationError> {
        let email = email.as_ref();
        if email.validate_email() {
            Ok(Self(normalize_email(email)))
        } else {
            Err(ValidationError::new("email"))
        }
    }

    /// Get the email as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert into the inner String.
    pub fn into_inner(self) -> String {
        self.0
    }

    pub fn hash(&self) -> String {
        hash_email(self.as_str())
    }
}

impl Deref for Email {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Deserialize with validation and normalization
impl<'de> Deserialize<'de> for Email {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let email = String::deserialize(deserializer)?;
        Email::new(&email).map_err(|_| serde::de::Error::custom(format!("Invalid email address: {}", email)))
    }
}

// Serialize as plain string
impl Serialize for Email {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_normalization() {
        let email = Email::new("Test@Example.COM").unwrap();
        assert_eq!(email.as_str(), "test@example.com");
    }

    #[test]
    fn test_email_validation() {
        assert!(Email::new("valid@example.com").is_ok());
        assert!(Email::new("VALID@EXAMPLE.COM").is_ok());
        assert!(Email::new("invalid").is_err());
        assert!(Email::new("").is_err());
        assert!(Email::new("@example.com").is_err());
    }

    #[test]
    fn test_email_deserialization() {
        let json = r#""Test@Example.COM""#;
        let email: Email = serde_json::from_str(json).unwrap();
        assert_eq!(email.as_str(), "test@example.com");
    }

    #[test]
    fn test_email_serialization() {
        let email = Email::new("test@example.com").unwrap();
        let json = serde_json::to_string(&email).unwrap();
        assert_eq!(json, r#""test@example.com""#);
    }

    #[test]
    fn test_invalid_email_deserialization() {
        let json = r#""invalid-email""#;
        let result: Result<Email, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
