use serde::Deserialize;

/// Serde deserializer for `Option<Vec<T>>` from an optional comma-separated query string value.
///
/// - Missing or empty → `None`
/// - `"a,b,c"` → `Some(vec![a, b, c])` where each token is parsed via `FromStr`
///
/// Use with `#[serde(default, deserialize_with = "shine_infra::serde::deserialize_optional_comma_list")]`.
pub fn deserialize_optional_comma_list<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    match raw {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => s
            .split(',')
            .map(|v| v.trim().parse::<T>().map_err(serde::de::Error::custom))
            .collect::<Result<Vec<T>, _>>()
            .map(Some),
    }
}
