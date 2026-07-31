use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
#[error("Connection string parameter {name:?} has an invalid value {value:?} (expected an integer)")]
pub struct CnsParamError {
    pub name: String,
    pub value: String,
}

/// Extract and strip a custom parameter from a connection string.
/// Returns (parsed_value, cleaned_connection_string); native drivers don't recognize the custom
/// parameters, so they are removed. A present-but-unparseable value is an error rather than being
/// silently dropped (which would let an operator's setting be ignored).
pub(crate) fn extract_and_strip_param(cns: &str, param_name: &str) -> Result<(Option<u64>, String), CnsParamError> {
    let Some(query_start) = cns.find('?') else {
        return Ok((None, cns.to_string()));
    };

    let base = &cns[..query_start];
    let query = &cns[query_start + 1..];
    let prefix = format!("{param_name}=");

    let mut value = None;
    let mut invalid = None;
    let filtered_params: Vec<&str> = query
        .split('&')
        .filter(|param| {
            if let Some(val_str) = param.strip_prefix(&prefix) {
                match val_str.parse::<u64>() {
                    Ok(v) => value = Some(v),
                    Err(_) => invalid = Some(val_str.to_string()),
                }
                false
            } else {
                true
            }
        })
        .collect();

    if let Some(value) = invalid {
        return Err(CnsParamError {
            name: param_name.to_string(),
            value,
        });
    }

    let cleaned_cns = if filtered_params.is_empty() {
        base.to_string()
    } else {
        format!("{}?{}", base, filtered_params.join("&"))
    };

    Ok((value, cleaned_cns))
}

#[cfg(test)]
mod test {
    use super::extract_and_strip_param;

    #[test]
    fn missing_query_returns_none() {
        let (value, cns) = extract_and_strip_param("postgres://host/db", "pool_timeout").unwrap();
        assert_eq!(value, None);
        assert_eq!(cns, "postgres://host/db");
    }

    #[test]
    fn absent_param_is_kept_untouched() {
        let (value, cns) = extract_and_strip_param("postgres://host/db?sslmode=disable", "pool_timeout").unwrap();
        assert_eq!(value, None);
        assert_eq!(cns, "postgres://host/db?sslmode=disable");
    }

    #[test]
    fn present_param_is_parsed_and_stripped() {
        let (value, cns) =
            extract_and_strip_param("postgres://host/db?sslmode=disable&pool_timeout=5", "pool_timeout").unwrap();
        assert_eq!(value, Some(5));
        assert_eq!(cns, "postgres://host/db?sslmode=disable");
    }

    #[test]
    fn only_param_leaves_no_query() {
        let (value, cns) = extract_and_strip_param("redis://host?pool_timeout=5000", "pool_timeout").unwrap();
        assert_eq!(value, Some(5000));
        assert_eq!(cns, "redis://host");
    }

    #[test]
    fn unparseable_param_is_an_error_not_a_silent_default() {
        let err = extract_and_strip_param("redis://host?pool_timeout=5s", "pool_timeout").unwrap_err();
        assert_eq!(err.name, "pool_timeout");
        assert_eq!(err.value, "5s");
    }
}
