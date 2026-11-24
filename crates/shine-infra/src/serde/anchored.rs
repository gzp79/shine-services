use crate::web::compile_anchored_bytes;
use regex::bytes::Regex;
use serde::{de::Error as SerdeError, Deserializer, Serialize, Serializer};

/// Serializes a vector of anchored byte regexes to their string patterns.
///
/// # Errors
/// Returns a serde error if serialization fails.
pub fn serialize<S>(regexes: &[Regex], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let patterns: Vec<String> = regexes.iter().map(|r| r.as_str().to_string()).collect();
    patterns.serialize(serializer)
}

/// Deserializes a vector of regex patterns into anchored byte regexes.
/// Each pattern must be fully anchored (^…$).
///
/// # Errors
/// Returns a serde error if any pattern is invalid or not anchored.
pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    let patterns: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    patterns
        .iter()
        .map(|r| {
            compile_anchored_bytes(r).map_err(|err| SerdeError::custom(format!("Anchored regex config error: {err}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    struct TestConfig {
        #[serde(serialize_with = "serialize", deserialize_with = "deserialize")]
        patterns: Vec<Regex>,
    }

    #[test]
    fn deserialize_valid_anchored_patterns() {
        let json = r#"{"patterns": ["^https://example\\.com$", "^ws\\.example\\.com(:443)?$"]}"#;
        let config: TestConfig = serde_json::from_str(json).expect("should deserialize valid patterns");
        assert_eq!(config.patterns.len(), 2);
    }

    #[test]
    fn deserialize_empty_patterns() {
        let json = r#"{"patterns": []}"#;
        let config: TestConfig = serde_json::from_str(json).expect("should deserialize empty patterns");
        assert!(config.patterns.is_empty());
    }

    #[test]
    fn deserialize_unanchored_pattern_fails() {
        let json = r#"{"patterns": ["https://example\\.com"]}"#;
        let result: Result<TestConfig, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn serialize_patterns() {
        let json = r#"{"patterns": ["^https://example\\.com$"]}"#;
        let config: TestConfig = serde_json::from_str(json).expect("should deserialize");
        let serialized = serde_json::to_string(&config).expect("should serialize");
        let re_deserialized: TestConfig = serde_json::from_str(&serialized).expect("should re-deserialize");
        assert_eq!(config.patterns.len(), re_deserialized.patterns.len());
    }
}
