use std::collections::HashMap;
use std::sync::OnceLock;

type NormStep = fn(&mut String);

struct ProviderRule {
    steps: &'static [NormStep],
    canonical_domain: Option<&'static str>,
}

fn strip_plus_tag(local: &mut String) {
    if let Some(pos) = local.find('+') {
        local.truncate(pos);
    }
}

fn strip_dash_tag(local: &mut String) {
    if let Some(pos) = local.find('-') {
        local.truncate(pos);
    }
}

fn remove_dots(local: &mut String) {
    local.retain(|c| c != '.');
}

/// Gmail normalization rules. Steps: strip plus-tag first, then remove dots.
/// Domain alias: googlemail.com → gmail.com
/// Spec: https://support.google.com/mail/answer/7436150
static GMAIL_RULE: ProviderRule = ProviderRule {
    steps: &[strip_plus_tag, remove_dots],
    canonical_domain: Some("gmail.com"),
};

/// Yahoo normalization rules. Uses dash-tag for disposable addresses.
/// Spec: https://help.yahoo.com/kb/SLN35441.html
static YAHOO_RULE: ProviderRule = ProviderRule {
    steps: &[strip_dash_tag],
    canonical_domain: None,
};

/// Outlook/Hotmail/Live normalization rules.
/// Spec: https://support.microsoft.com/en-us/office/plus-addressing-in-outlook
static OUTLOOK_RULE: ProviderRule = ProviderRule {
    steps: &[strip_plus_tag],
    canonical_domain: None,
};

static DEFAULT_RULE: ProviderRule = ProviderRule {
    steps: &[],
    canonical_domain: None,
};

fn registry() -> &'static HashMap<&'static str, &'static ProviderRule> {
    static REGISTRY: OnceLock<HashMap<&'static str, &'static ProviderRule>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("gmail.com", &GMAIL_RULE);
        m.insert("googlemail.com", &GMAIL_RULE);
        m.insert("yahoo.com", &YAHOO_RULE);
        m.insert("yahoo.co.uk", &YAHOO_RULE);
        m.insert("ymail.com", &YAHOO_RULE);
        m.insert("outlook.com", &OUTLOOK_RULE);
        m.insert("hotmail.com", &OUTLOOK_RULE);
        m.insert("live.com", &OUTLOOK_RULE);
        m
    })
}

/// Returns (raw, normalized):
///   raw        = trimmed + lowercased; use for display and sending
///   normalized = raw + provider steps + canonical domain rewrite; use for uniqueness and lookup
pub(super) fn normalize_email(input: &str) -> (String, String) {
    let lowered = input.trim().to_lowercase();

    let at = match lowered.rfind('@') {
        Some(pos) => pos,
        None => return (lowered.clone(), lowered),
    };

    let local = &lowered[..at];
    let domain = &lowered[at + 1..];
    let raw = lowered.clone();

    let rule = registry().get(domain).copied().unwrap_or(&DEFAULT_RULE);

    let mut norm_local = local.to_string();
    for step in rule.steps {
        step(&mut norm_local);
    }
    let norm_domain = rule.canonical_domain.unwrap_or(domain);
    let normalized = format!("{}@{}", norm_local, norm_domain);

    (raw, normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(input: &str, expected_raw: &str, expected_normalized: &str) {
        let (raw, normalized) = normalize_email(input);
        assert_eq!(raw, expected_raw, "raw mismatch for input {input:?}");
        assert_eq!(
            normalized, expected_normalized,
            "normalized mismatch for input {input:?}"
        );
    }

    #[test]
    fn test_default() {
        check("  USER@EXAMPLE.COM  ", "user@example.com", "user@example.com");
        check("User+tag@example.com", "user+tag@example.com", "user+tag@example.com");
        check("u.ser@example.com", "u.ser@example.com", "u.ser@example.com");
    }

    #[test]
    fn test_google() {
        check("U.ser+tag@Googlemail.Com", "u.ser+tag@googlemail.com", "user@gmail.com");
        check("u.na+me@gmail.com", "u.na+me@gmail.com", "una@gmail.com");
        check("u.ser+tag@gmail.com", "u.ser+tag@gmail.com", "user@gmail.com");
    }

    #[test]
    fn test_yahoo() {
        check("user-tag@yahoo.com", "user-tag@yahoo.com", "user@yahoo.com");
        check("user-tag@yahoo.co.uk", "user-tag@yahoo.co.uk", "user@yahoo.co.uk");
        check("user-tag@ymail.com", "user-tag@ymail.com", "user@ymail.com");
    }

    #[test]
    fn test_outlook() {
        check("user+tag@outlook.com", "user+tag@outlook.com", "user@outlook.com");
        check("user+tag@hotmail.com", "user+tag@hotmail.com", "user@hotmail.com");
        check("user+tag@live.com", "user+tag@live.com", "user@live.com");
    }
}
