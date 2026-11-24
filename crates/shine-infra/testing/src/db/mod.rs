pub mod postgres;
pub mod redis;

/// The connection string with the extra parameters appended, to give a test pool timeouts of its own.
pub fn with_params(cns: &str, params: &str) -> String {
    if params.is_empty() {
        return cns.to_string();
    }
    let separator = if cns.contains('?') { '&' } else { '?' };
    format!("{cns}{separator}{params}")
}
