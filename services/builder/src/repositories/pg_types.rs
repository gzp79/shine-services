//! Postgres representations of the domain types the repositories share.

use crate::repositories::command_inbox::InboxId;
use bytes::BytesMut;
use postgres_types::{to_sql_checked, FromSql, IsNull, ToSql, Type};
use std::error::Error;

#[derive(Clone, Copy, Debug)]
pub struct StoredInboxId(pub InboxId);

impl ToSql for StoredInboxId {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        self.0.to_string().to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        <String as ToSql>::accepts(ty)
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for StoredInboxId {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let text = <&str as FromSql>::from_sql(ty, raw)?;
        InboxId::parse(text)
            .map(Self)
            .ok_or_else(|| format!("not an inbox id: {text:?}").into())
    }

    fn accepts(ty: &Type) -> bool {
        <&str as FromSql>::accepts(ty)
    }
}
