use std::{collections::HashMap, env};

use config::{ConfigError, Map, Source, Value, ValueKind};

/// Base on config-rs crate, but with some modifications
#[derive(Debug, Clone, Default)]
pub struct Environment {
    extra_conversion: HashMap<String, String>,
}

impl Environment {
    pub fn new() -> Self {
        Self::default()
    }

    fn convert_key_case(&self, key: &str) -> String {
        key.split("_")
            .enumerate()
            .map(|(i, s)| {
                if let Some(key) = self.extra_conversion.get(s) {
                    key.to_string()
                } else if i == 0 {
                    s.to_lowercase()
                } else {
                    let mut chars = s.chars();
                    chars
                        .next()
                        .map(|c| c.to_uppercase().collect::<String>())
                        .unwrap_or_default()
                        + &chars.as_str().to_lowercase()
                }
            })
            .collect()
    }
}

impl Source for Environment {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new((*self).clone())
    }

    fn collect(&self) -> Result<Map<String, Value>, ConfigError> {
        let uri: String = "environment".into();
        let prefix = "shine--";
        let separator = "--";
        let try_parsing = false;
        let ignore_empty = false;
        let list_separator: Option<String> = None;
        let list_parse_keys: Option<Vec<String>> = None;

        let mut m = Map::new();
        let collector = |(env_key, value): (String, String)| {
            //log::trace!("Checking env {} ...", env_key);

            // Treat empty environment variables as unset
            if ignore_empty && value.is_empty() {
                return;
            }

            let mut key = env_key.to_lowercase();
            key = match key.strip_prefix(prefix) {
                None => return,
                Some(key) => key.to_owned(),
            };
            key = key
                .split(separator)
                .map(|k| self.convert_key_case(k))
                .collect::<Vec<_>>()
                .join(".");
            log::debug!("Reading env {} as {}...", env_key, key);

            let value = if try_parsing {
                // convert to lowercase because bool parsing expects all lowercase
                if let Ok(parsed) = value.to_lowercase().parse::<bool>() {
                    ValueKind::Boolean(parsed)
                } else if let Ok(parsed) = value.parse::<i64>() {
                    ValueKind::I64(parsed)
                } else if let Ok(parsed) = value.parse::<f64>() {
                    ValueKind::Float(parsed)
                } else if let Some(separator) = &list_separator {
                    if let Some(keys) = &list_parse_keys {
                        if keys.contains(&key) {
                            let v: Vec<Value> = value
                                .split(separator)
                                .map(|s| Value::new(Some(&uri), ValueKind::String(s.to_owned())))
                                .collect();
                            ValueKind::Array(v)
                        } else {
                            ValueKind::String(value)
                        }
                    } else {
                        let v: Vec<Value> = value
                            .split(separator)
                            .map(|s| Value::new(Some(&uri), ValueKind::String(s.to_owned())))
                            .collect();
                        ValueKind::Array(v)
                    }
                } else {
                    ValueKind::String(value)
                }
            } else {
                ValueKind::String(value)
            };

            m.insert(key, Value::new(Some(&uri), value));
        };

        env::vars().for_each(collector);

        log::trace!("Environment: {:#?}", m);
        Ok(m)
    }
}
