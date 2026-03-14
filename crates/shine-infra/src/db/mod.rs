mod cacerts;
pub use self::cacerts::*;
mod db_error;
pub use self::db_error::*;
mod redis;
pub use self::redis::*;
mod postgres;
pub use self::postgres::*;

pub mod event_source;

/// Extract and strip a custom parameter from a connection string
/// Returns (parsed_value, cleaned_connection_string)
///
/// Custom parameters are removed from the returned connection string since
/// native database drivers don't recognize them.
pub(crate) fn extract_and_strip_param(cns: &str, param_name: &str) -> (Option<u64>, String) {
    let Some(query_start) = cns.find('?') else {
        return (None, cns.to_string());
    };

    let base = &cns[..query_start];
    let query = &cns[query_start + 1..];
    let prefix = format!("{param_name}=");

    let mut value = None;
    let filtered_params: Vec<&str> = query
        .split('&')
        .filter(|param| {
            if let Some(val_str) = param.strip_prefix(&prefix) {
                value = val_str.parse::<u64>().ok();
                false
            } else {
                true
            }
        })
        .collect();

    let cleaned_cns = if filtered_params.is_empty() {
        base.to_string()
    } else {
        format!("{}?{}", base, filtered_params.join("&"))
    };

    (value, cleaned_cns)
}
