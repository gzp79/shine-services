use crate::{
    models::{Identity, IdentityError, SearchIdentity, MAX_SEARCH_RESULT_COUNT},
    repositories::identity::{pg::PgIdentityDbContext, IdentitySearch},
};
use postgres_from_row::FromRow;
use shine_infra::db::{DBError, QueryBuilder};
use tracing::instrument;

use super::pg_identities::IdentityRow;

impl IdentitySearch for PgIdentityDbContext<'_> {
    #[instrument(skip(self))]
    async fn search_identity(&mut self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError> {
        let mut builder =
            QueryBuilder::new("SELECT user_id, kind, name, encrypted_email, email_confirmed, created FROM identities");

        let name_patterns: Vec<String> = search
            .names
            .iter()
            .flat_map(|names| names.iter().map(|n| format!("%{n}%")))
            .collect();
        let email_hashes: Vec<String> = search
            .emails
            .iter()
            .flat_map(|emails| {
                emails
                    .iter()
                    .map(|e| self.email_protection.hash(e).as_str().to_string())
            })
            .collect();

        if let Some(user_ids) = &search.user_ids {
            builder.and_where(|b| format!("user_id = ANY(${b})"), [user_ids]);
        }

        if search.names.is_some() {
            builder.and_where(|b| format!("name ILIKE ANY(${b})"), [&name_patterns]);
        }

        if search.emails.is_some() {
            builder.and_where(|b| format!("email_hash = ANY(${b})"), [&email_hashes]);
        }

        builder.order_by("name");
        builder.order_by("user_id");

        let count = usize::min(MAX_SEARCH_RESULT_COUNT, search.count.unwrap_or(MAX_SEARCH_RESULT_COUNT));
        builder.limit(count);

        let (stmt, params) = builder.build();
        let rows = self.client.query(&stmt, &params).await.map_err(DBError::from)?;

        let identities = rows
            .into_iter()
            .map(|row| IdentityRow::from_row(&row).into_identity(self.email_protection))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(identities)
    }
}
