use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    En,
    Hu,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Language::En => write!(f, "en"),
            Language::Hu => write!(f, "hu"),
        }
    }
}
