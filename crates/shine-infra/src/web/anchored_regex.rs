use regex_syntax::hir::Look;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum AnchoredRegexError {
    #[error("Invalid regex pattern {0:?}: {1}")]
    Invalid(String, String),
    #[error("Pattern {0:?} must be fully anchored (^…$)")]
    NotAnchored(String),
}

/// Returns whether every possible match of `pattern` starts at `^`/`\A` and ends at `$`/`\z`.
pub fn is_anchored(pattern: &str) -> Result<bool, AnchoredRegexError> {
    let hir = regex_syntax::parse(pattern)
        .map_err(|err| AnchoredRegexError::Invalid(pattern.to_string(), err.to_string()))?;
    let props = hir.properties();
    Ok(props.look_set_prefix().contains(Look::Start) && props.look_set_suffix().contains(Look::End))
}

/// Compiles a `regex::Regex`, rejecting any pattern that is not fully anchored (`^…$`).
pub fn compile_anchored(pattern: &str) -> Result<regex::Regex, AnchoredRegexError> {
    if !is_anchored(pattern)? {
        return Err(AnchoredRegexError::NotAnchored(pattern.to_string()));
    }
    regex::Regex::new(pattern).map_err(|err| AnchoredRegexError::Invalid(pattern.to_string(), err.to_string()))
}

/// Like [`compile_anchored`] but produces a byte regex (`regex::bytes::Regex`).
pub fn compile_anchored_bytes(pattern: &str) -> Result<regex::bytes::Regex, AnchoredRegexError> {
    if !is_anchored(pattern)? {
        return Err(AnchoredRegexError::NotAnchored(pattern.to_string()));
    }
    regex::bytes::Regex::new(pattern).map_err(|err| AnchoredRegexError::Invalid(pattern.to_string(), err.to_string()))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn accepts_anchored_patterns() {
        for pattern in [
            r"^https://cloud\.scytta\.com$",
            r"^ws\.scytta\.com(:443)?$",
            r"^https://([a-zA-Z0-9-]+\.)+local.scytta\.com(:\d+)?$",
            r"^a$|^b$",
            r"\Aabc\z",
        ] {
            assert!(is_anchored(pattern).unwrap(), "should be anchored: {pattern:?}");
            assert!(compile_anchored(pattern).is_ok());
            assert!(compile_anchored_bytes(pattern).is_ok());
        }
    }

    #[test]
    fn rejects_unanchored_patterns() {
        for pattern in [
            r"https://app\.example\.com",  // no anchors
            r"^https://app\.example\.com", // start only
            r"https://app\.example\.com$", // end only
            r"^a$|b",                      // one arm unanchored
        ] {
            assert!(!is_anchored(pattern).unwrap(), "should NOT be anchored: {pattern:?}");
            assert!(matches!(
                compile_anchored(pattern),
                Err(AnchoredRegexError::NotAnchored(_))
            ));
            assert!(matches!(
                compile_anchored_bytes(pattern),
                Err(AnchoredRegexError::NotAnchored(_))
            ));
        }
    }

    #[test]
    fn reports_invalid_pattern() {
        assert!(matches!(is_anchored(r"("), Err(AnchoredRegexError::Invalid(..))));
    }
}
