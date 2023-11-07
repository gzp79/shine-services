use crate::db::{Identity, IdentityError};
use shine_service::service::{PGConnection, PGRawConnection, QueryBuilder};
use tokio_postgres::Row;
use uuid::Uuid;

pub const MAX_SEARCH_COUNT: usize = 100;

#[derive(Debug)]
pub enum SearchIdentityOrder {
    UserId(Option<Uuid>),
    Email(Option<(String, Uuid)>),
    Name(Option<(String, Uuid)>),
}

#[derive(Debug)]
pub struct SearchIdentity<'a> {
    pub order: SearchIdentityOrder,
    pub count: Option<usize>,

    pub user_ids: Option<&'a [Uuid]>,
    pub emails: Option<&'a [String]>,
    pub names: Option<&'a [String]>,
}

/// Identities Data Access Object.
pub struct IdentitySearchDAO<'a, T>
where
    T: PGRawConnection,
{
    client: &'a PGConnection<T>,
}

impl<'a, T> IdentitySearchDAO<'a, T>
where
    T: PGRawConnection,
{
    pub fn new(client: &'a PGConnection<T>) -> Self {
        Self { client }
    }

    pub async fn search(&self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError> {
        log::info!("{search:?}");
        let mut builder = QueryBuilder::new(
            "SELECT user_id, kind, name, email, email_confirmed, created, data_version FROM identities",
        );

        fn into_identity(r: Row) -> Result<Identity, IdentityError> {
            Ok(Identity {
                id: r.try_get(0)?,
                kind: r.try_get(1)?,
                name: r.try_get(2)?,
                email: r.try_get(3)?,
                is_email_confirmed: r.try_get(4)?,
                created: r.try_get(5)?,
                version: r.try_get(6)?,
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

        let count = usize::min(MAX_SEARCH_COUNT, search.count.unwrap_or(MAX_SEARCH_COUNT));
        builder.limit(count);

        let (stmt, params) = builder.build();
        log::info!("{stmt:?}");
        let rows = self.client.query(&stmt, &params).await?;

        let identities = rows.into_iter().map(into_identity).collect::<Result<Vec<_>, _>>()?;
        Ok(identities)
    }
}
