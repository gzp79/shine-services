use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum TokenKind {
    Access,
    Persistent,
    SingleAccess,
    EmailAccess,
}

impl TokenKind {
    // return if token can be used only once
    pub fn is_single_access(&self) -> bool {
        matches!(self, Self::SingleAccess) || matches!(self, Self::EmailAccess)
    }

    // return if only one token of this kind can exist per user
    pub fn is_unique(&self) -> bool {
        matches!(self, Self::EmailAccess)
    }

    pub fn all() -> &'static [TokenKind] {
        &[
            TokenKind::Access,
            TokenKind::Persistent,
            TokenKind::SingleAccess,
            TokenKind::EmailAccess,
        ]
    }

    pub fn all_single_access() -> &'static [TokenKind] {
        &[TokenKind::SingleAccess, TokenKind::EmailAccess]
    }

    pub fn all_multi_access() -> &'static [TokenKind] {
        &[TokenKind::Access, TokenKind::Persistent]
    }
}
