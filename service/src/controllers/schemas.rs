use std::borrow::Cow;

use crate::repositories::identity::TokenKind;
use utoipa::{
    openapi::{
        schema::{Schema, SchemaType},
        Object, RefOr, Type,
    },
    PartialSchema, ToSchema,
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
