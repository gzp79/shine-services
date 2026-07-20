use crate::email::normalizer::normalize_email;
use hex;
use ring::digest;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use utoipa::{
    openapi::{schema::Schema, KnownFormat, ObjectBuilder, SchemaFormat, Type},
    PartialSchema, ToSchema,
};
use validator::ValidateEmail;
use validator::ValidationError;

fn hash_email(normalized: &str) -> String {
    let hash = digest::digest(&digest::SHA256, normalized.as_bytes());

    hex::encode(hash)
}

/// Validated email address carrying both raw (trimmed+lowercased) and
/// provider-normalized forms.
///
/// - Use `.raw()` for sending email and display.
/// - Use `.normalized()` and `.hash()` for DB uniqueness and lookups.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Email {
    raw: String,
    normalized: String,
}

impl PartialSchema for Email {
    fn schema() -> utoipa::openapi::RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::String)
            .format(Some(SchemaFormat::KnownFormat(KnownFormat::Email)))
            .examples(Some(serde_json::Value::String("user@example.com".to_string())))
            .build()
            .into()
    }
}

impl ToSchema for Email {}

impl Email {
    /// Validate, trim, lowercase, and provider-normalize an email address.
    pub fn new(input: impl AsRef<str>) -> Result<Self, ValidationError> {
        let input = input.as_ref();
        let trimmed = input.trim().to_lowercase();
        if !trimmed.validate_email() {
            return Err(ValidationError::new("email"));
        }
        let (raw, normalized) = normalize_email(input);
        Ok(Self { raw, normalized })
    }

    /// Reconstruct an Email from already-stored raw and normalized strings (no re-normalization).
    pub fn from_parts(raw: String, normalized: String) -> Self {
        Self { raw, normalized }
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn to_raw(self) -> String {
        self.raw
    }

    pub fn normalized(&self) -> &str {
        &self.normalized
    }

    pub fn hash(&self) -> String {
        hash_email(&self.normalized)
    }

    pub fn raw_hash(&self) -> String {
        hash_email(&self.raw)
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.raw
    }
}

impl<'de> Deserialize<'de> for Email {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Email::new(&s).map_err(|_| serde::de::Error::custom(format!("Invalid email address: {s}")))
    }
}

impl Serialize for Email {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_raw_is_lowercase_trimmed() {
        let e = Email::new("  User.Na+me@Gmail.Com  ").unwrap();
        assert_eq!(e.raw(), "user.na+me@gmail.com");
    }

    #[test]
    fn test_email_normalized_applies_provider_steps() {
        let e = Email::new("  User.Na+me@Gmail.Com  ").unwrap();
        assert_eq!(e.normalized(), "userna@gmail.com");
    }

    #[test]
    fn test_email_hash_is_of_normalized() {
        let base = Email::new("user@gmail.com").unwrap();
        let alias = Email::new("user+tag@gmail.com").unwrap();
        assert_eq!(alias.hash(), base.hash());
    }

    #[test]
    fn test_email_raw_hash_differs_between_variants() {
        let base = Email::new("user@gmail.com").unwrap();
        let alias = Email::new("user+tag@gmail.com").unwrap();
        assert_eq!(alias.hash(), base.hash());
        assert_ne!(alias.raw_hash(), base.raw_hash());
    }

    #[test]
    fn test_email_display_uses_raw() {
        let e = Email::new("User+tag@Gmail.Com").unwrap();
        assert_eq!(format!("{e}"), "user+tag@gmail.com");
    }

    #[test]
    fn test_email_serialize_uses_raw() {
        let e = Email::new("User+tag@Gmail.Com").unwrap();
        assert_eq!(serde_json::to_string(&e).unwrap(), r#""user+tag@gmail.com""#);
    }

    #[test]
    fn test_email_deserialize_validates_and_normalizes() {
        let e: Email = serde_json::from_str(r#""User+tag@Gmail.Com""#).unwrap();
        assert_eq!(e.raw(), "user+tag@gmail.com");
        assert_eq!(e.normalized(), "user@gmail.com");
    }

    #[test]
    fn test_invalid_email_rejected() {
        assert!(Email::new("invalid").is_err());
        assert!(Email::new("").is_err());
        assert!(Email::new("@example.com").is_err());
    }

    #[test]
    fn test_from_parts_no_re_normalization() {
        let e = Email::from_parts("user+tag@gmail.com".into(), "user@gmail.com".into());
        assert_eq!(e.raw(), "user+tag@gmail.com");
        assert_eq!(e.normalized(), "user@gmail.com");
    }
}
