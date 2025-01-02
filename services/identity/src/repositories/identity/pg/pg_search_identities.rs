use crate::repositories::{
    identity::{Identity, IdentityError, IdentitySearch, SearchIdentity, SearchIdentityOrder, MAX_SEARCH_RESULT_COUNT},
    DBError,
};
use shine_core::db::QueryBuilder;
use tokio_postgres::Row;
use tracing::instrument;

use super::PgIdentityDbContext;

impl<'a> IdentitySearch for PgIdentityDbContext<'a> {
    #[instrument(skip(self))]
    async fn search_identity(&mut self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError> {
        log::info!("{search:?}");
        let mut builder = QueryBuilder::new(
            "SELECT user_id, kind, name, email, email_confirmed, created, data_version FROM identities",
        );

        fn into_identity(r: Row) -> Result<Identity, IdentityError> {
            Ok(Identity {
                id: r.try_get(0).map_err(DBError::from)?,
                kind: r.try_get(1).map_err(DBError::from)?,
                name: r.try_get(2).map_err(DBError::from)?,
                email: r.try_get(3).map_err(DBError::from)?,
                is_email_confirmed: r.try_get(4).map_err(DBError::from)?,
                created: r.try_get(5).map_err(DBError::from)?,
                version: r.try_get(6).map_err(DBError::from)?,
            })
        }

        if let Some(user_ids) = &search.user_ids {
            builder.and_where(|b| format!("user_id = ANY(${b})"), [user_ids]);
        }

        if let Some(names) = &search.names {
            builder.and_where(|b| format!("name = ANY(${b})"), [names]);
        }

        if let Some(emails) = &search.emails {
            builder.and_where(|b| format!("email = ANY(${b})"), [emails]);
        }

        match &search.order {
            SearchIdentityOrder::UserId(start) => {
                if let Some(user_id) = start {
                    builder.and_where(|b| format!("user_id > ${b}"), [user_id]);
                }
            }
            SearchIdentityOrder::Email(start) => {
                if let Some((email, user_id)) = start {
                    builder.and_where(
                        |b1, b2| format!("(email > ${b1} OR (email == ${b1} AND user_id > ${b2}))"),
                        [email, user_id],
                    );
                }
                builder.order_by("email");
            }
            SearchIdentityOrder::Name(start) => {
                if let Some((name, user_id)) = start {
                    builder.and_where(
                        |b1, b2| format!("(name > ${b1} OR (name == ${b1} AND user_id > ${b2}))"),
                        [name, user_id],
                    );
                }
                builder.order_by("name");
            }
        };
        builder.order_by("user_id");

        let count = usize::min(MAX_SEARCH_RESULT_COUNT, search.count.unwrap_or(MAX_SEARCH_RESULT_COUNT));
        builder.limit(count);

        let (stmt, params) = builder.build();
        log::info!("{stmt:?}");
        let rows = self.client.query(&stmt, &params).await.map_err(DBError::from)?;

        let identities = rows.into_iter().map(into_identity).collect::<Result<Vec<_>, _>>()?;
        Ok(identities)
    }
}
