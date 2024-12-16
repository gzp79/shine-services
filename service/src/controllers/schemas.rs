use std::borrow::Cow;

use crate::repositories::identity::{IdentityKind, TokenKind};
use utoipa::{
    openapi::{
        schema::{Schema, SchemaType},
        Object, RefOr, Type,
    },
    PartialSchema,
};

impl PartialSchema for TokenKind {
    fn schema() -> RefOr<Schema> {
        Object::builder()
            .schema_type(SchemaType::new(Type::String))
            .enum_values(Some(["singleAccess", "persistent", "access"]))
            .into()
    }
}

impl utoipa::ToSchema for TokenKind {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("TokenKind")
    }
}

impl PartialSchema for IdentityKind {
    fn schema() -> RefOr<Schema> {
        Object::builder()
            .schema_type(SchemaType::new(Type::String))
            .enum_values(Some(["user", "studio"]))
            .into()
    }
}

impl utoipa::ToSchema for IdentityKind {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("IdentityKind")
    }
}
