use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum CnsParamError {
    #[error("Connection string parameter {name:?} has an invalid value {value:?} (expected an integer)")]
    InvalidValue { name: String, value: String },
    #[error("Connection string parameter {name:?} is specified more than once")]
    Duplicate { name: String },
}

/// A connection string parsed into its base and query parameters, letting custom parameters be
/// taken out before the cleaned string is handed to a native driver that wouldn't recognize them.
pub struct ConnectionString<'a> {
    base: &'a str,
    params: Vec<&'a str>,
}

impl<'a> ConnectionString<'a> {
    pub fn parse(cns: &'a str) -> Self {
        let (base, query) = split_query(cns);
        let params = query.map(|q| q.split('&').collect()).unwrap_or_default();
        Self { base, params }
    }

    /// Removes the named parameter and returns its value, or `None` if absent. A present but
    /// non-integer value is an error rather than a silent default, so an operator's setting can't be
    /// ignored; a repeated parameter is an error too, since which occurrence wins would otherwise be
    /// ambiguous.
    pub fn take_u64(&mut self, name: &str) -> Result<Option<u64>, CnsParamError> {
        let prefix = format!("{name}=");
        let mut matches = self.params.iter().enumerate().filter(|(_, p)| p.starts_with(&prefix));
        let Some((pos, _)) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            return Err(CnsParamError::Duplicate { name: name.to_string() });
        }
        let value = &self.params.remove(pos)[prefix.len()..];
        value.parse::<u64>().map(Some).map_err(|_| CnsParamError::InvalidValue {
            name: name.to_string(),
            value: value.to_string(),
        })
    }

    /// The connection string with every taken parameter removed.
    pub fn into_cns(self) -> String {
        if self.params.is_empty() {
            self.base.to_string()
        } else {
            format!("{}?{}", self.base, self.params.join("&"))
        }
    }
}

/// Splits a connection string into its base and query, as `(base, Some(query))`, or `(cns, None)`
/// when there is no query.
pub fn split_query(cns: &str) -> (&str, Option<&str>) {
    // Skip the userinfo so a `?` inside it (e.g. a password) isn't taken for the separator. Userinfo
    // ends at its `@`; a raw `?` is allowed there but not in the host, so only `/` and `#` bound the
    // `@` search.
    let scan_from = match cns.find("://") {
        Some(scheme_end) => {
            let authority = scheme_end + 3;
            let userinfo_end = cns[authority..].find(['/', '#']).map_or(cns.len(), |i| authority + i);
            match cns[authority..userinfo_end].find('@') {
                Some(at) => authority + at + 1,
                None => authority,
            }
        }
        None => 0,
    };
    match cns[scan_from..].find('?') {
        Some(i) => (&cns[..scan_from + i], Some(&cns[scan_from + i + 1..])),
        None => (cns, None),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn split_query_separates_base_and_query() {
        assert_eq!(split_query("redis://host"), ("redis://host", None));
        assert_eq!(split_query("redis://host?a=1&b=2"), ("redis://host", Some("a=1&b=2")));
        assert_eq!(
            split_query("redis://user:pa?ss@host?a=1"),
            ("redis://user:pa?ss@host", Some("a=1"))
        );
        assert_eq!(
            split_query("redis://user:pa?ss@host"),
            ("redis://user:pa?ss@host", None)
        );
    }

    #[test]
    fn missing_query_returns_none() {
        let mut cns = ConnectionString::parse("postgres://host/db");
        assert_eq!(cns.take_u64("pool_timeout").unwrap(), None);
        assert_eq!(cns.into_cns(), "postgres://host/db");
    }

    #[test]
    fn absent_param_is_kept_untouched() {
        let mut cns = ConnectionString::parse("postgres://host/db?sslmode=disable");
        assert_eq!(cns.take_u64("pool_timeout").unwrap(), None);
        assert_eq!(cns.into_cns(), "postgres://host/db?sslmode=disable");
    }

    #[test]
    fn present_param_is_parsed_and_stripped() {
        let mut cns = ConnectionString::parse("postgres://host/db?sslmode=disable&pool_timeout=5");
        assert_eq!(cns.take_u64("pool_timeout").unwrap(), Some(5));
        assert_eq!(cns.into_cns(), "postgres://host/db?sslmode=disable");
    }

    #[test]
    fn only_param_leaves_no_query() {
        let mut cns = ConnectionString::parse("redis://host?pool_timeout=5000");
        assert_eq!(cns.take_u64("pool_timeout").unwrap(), Some(5000));
        assert_eq!(cns.into_cns(), "redis://host");
    }

    #[test]
    fn multiple_params_taken_in_one_pass() {
        let mut cns = ConnectionString::parse("redis://host?pool_timeout=5000&max_size=20&keep=1");
        assert_eq!(cns.take_u64("pool_timeout").unwrap(), Some(5000));
        assert_eq!(cns.take_u64("max_size").unwrap(), Some(20));
        assert_eq!(cns.into_cns(), "redis://host?keep=1");
    }

    #[test]
    fn unparseable_param_is_an_error_not_a_silent_default() {
        let err = ConnectionString::parse("redis://host?pool_timeout=5s")
            .take_u64("pool_timeout")
            .unwrap_err();
        assert!(
            matches!(err, CnsParamError::InvalidValue { ref name, ref value } if name == "pool_timeout" && value == "5s"),
            "got {err:?}"
        );
    }

    #[test]
    fn duplicate_param_is_an_error_not_an_ambiguous_pick() {
        let err = ConnectionString::parse("redis://host?pool_timeout=5&pool_timeout=6")
            .take_u64("pool_timeout")
            .unwrap_err();
        assert!(
            matches!(err, CnsParamError::Duplicate { ref name } if name == "pool_timeout"),
            "got {err:?}"
        );
    }

    #[test]
    fn question_mark_in_credentials_is_not_the_query_separator() {
        let mut cns = ConnectionString::parse("redis://user:pa?ss@host?pool_timeout=5000");
        assert_eq!(cns.take_u64("pool_timeout").unwrap(), Some(5000));
        assert_eq!(cns.into_cns(), "redis://user:pa?ss@host");
    }

    #[test]
    fn question_mark_in_credentials_without_query_returns_none() {
        let mut cns = ConnectionString::parse("redis://user:pa?ss@host");
        assert_eq!(cns.take_u64("pool_timeout").unwrap(), None);
        assert_eq!(cns.into_cns(), "redis://user:pa?ss@host");
    }
}
