use crate::repositories::{
    identity::{
        ExternalLink, ExternalLinks, ExternalUserInfo, Identity, IdentityBuildError, IdentityError, IdentityKind,
    },
    DBError,
};
use chrono::{DateTime, Utc};
use futures::FutureExt;
use postgres_from_row::FromRow;
use shine_service::{
    pg_query,
    service::{PGClient, PGErrorChecks},
};
use tracing::instrument;
use uuid::Uuid;

use super::PgIdentityDbContext;

pg_query!( InsertExternalLogin =>
    in = user_id: Uuid, provider: &str, provider_id: &str, name: Option<&str>, email: Option<&str>;
    out = linked: DateTime<Utc>;
    sql = r#"
        INSERT INTO external_logins (user_id, provider, provider_id, name, email, linked) 
            VALUES ($1, $2, $3, $4, $5, now())
        RETURNING linked
    "#
);

#[derive(FromRow)]
struct FindByProviderIdRow {
    user_id: Uuid,
    kind: IdentityKind,
    name: String,
    email: Option<String>,
    email_confirmed: bool,
    created: DateTime<Utc>,
    data_version: i32,
}

pg_query!( FindByProviderId =>
    in = provider: &str, provider_id: &str;
    out = FindByProviderIdRow;
    sql = r#"
        SELECT i.user_id, i.kind, i.name, i.email, i.email_confirmed, i.created, i.data_version            
            FROM external_logins e, identities i
            WHERE e.user_id = i.user_id
                AND e.provider = $1
                AND e.provider_id = $2
    "#
);

#[derive(FromRow)]
struct ListByUserIdRow {
    user_id: Uuid,
    provider: String,
    provider_id: String,
    name: Option<String>,
    email: Option<String>,
    linked: DateTime<Utc>,
}

pg_query!( ListByUserId =>
    in = user_id: Uuid;
    out = ListByUserIdRow;
    sql = r#"
        SELECT e.user_id, e.provider, e.provider_id, e.name, e.email, e.linked
            FROM external_logins e
            WHERE e.user_id = $1
    "#
);

pg_query!( DeleteLink =>
    in = user_id: Uuid, provider: &str, provider_id: &str;
    sql = r#"
        DELETE FROM external_logins
            WHERE user_id = $1
                AND provider = $2
                AND provider_id = $3
    "#
);

pg_query!( ExistsByUserId =>
    in = user_id: Uuid;
    out = is_linked: bool;
    sql = r#"
        SELECT
            CASE WHEN EXISTS( SELECT 1 FROM external_logins e WHERE e.user_id = $1 ) THEN TRUE
            ELSE FALSE
            END as is_linked
    "#
);

#[derive(Clone)]
pub struct PgExternalLinksStatements {
    insert: InsertExternalLogin,
    find_by_provider_id: FindByProviderId,
    list_by_user_id: ListByUserId,
    exists_by_user_id: ExistsByUserId,
    delete_link: DeleteLink,
}

impl PgExternalLinksStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            insert: InsertExternalLogin::new(client).await.map_err(DBError::from)?,
            find_by_provider_id: FindByProviderId::new(client).await.map_err(DBError::from)?,
            list_by_user_id: ListByUserId::new(client).await.map_err(DBError::from)?,
            exists_by_user_id: ExistsByUserId::new(client).await.map_err(DBError::from)?,
            delete_link: DeleteLink::new(client).await.map_err(DBError::from)?,
        })
    }
}

impl<'a> ExternalLinks for PgIdentityDbContext<'a> {
    #[instrument(skip(self))]
    async fn link_user(&mut self, user_id: Uuid, external_user: &ExternalUserInfo) -> Result<(), IdentityError> {
        match self
            .stmts_external_links
            .insert
            .query_one(
                &self.client,
                &user_id,
                &external_user.provider.as_str(),
                &external_user.provider_id.as_str(),
                &external_user.name.as_deref(),
                &external_user.email.as_deref(),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.is_constraint("external_logins", "idx_provider_provider_id") {
                    Err(IdentityError::LinkProviderConflict)
                } else {
                    Err(IdentityError::DBError(err.into()))
                }
            }
        }
    }

    #[instrument(skip(self))]
    async fn find_all_links(&mut self, user_id: Uuid) -> Result<Vec<ExternalLink>, IdentityError> {
        let links = self
            .stmts_external_links
            .list_by_user_id
            .query(&self.client, &user_id)
            .await
            .map_err(DBError::from)?
            .into_iter()
            .map(|row| ExternalLink {
                user_id: row.user_id,
                provider: row.provider,
                provider_id: row.provider_id,
                name: row.name,
                email: row.email,
                linked_at: row.linked,
            })
            .collect();

        Ok(links)
    }

    #[instrument(skip(self))]
    async fn is_linked(&mut self, user_id: Uuid) -> Result<bool, IdentityError> {
        let is_linked = self
            .stmts_external_links
            .exists_by_user_id
            .query_one(&self.client, &user_id)
            .inspect(|d| log::info!("is_linked: {:?}", d))
            .await
            .map_err(DBError::from)?;

        Ok(is_linked)
    }

    #[instrument(skip(self))]
    async fn find_by_external_link(
        &mut self,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<Identity>, IdentityError> {
        Ok(self
            .stmts_external_links
            .find_by_provider_id
            .query_opt(&self.client, &provider, &provider_id)
            .await
            .map_err(DBError::from)?
            .map(|row| Identity {
                id: row.user_id,
                kind: row.kind,
                name: row.name,
                email: row.email,
                is_email_confirmed: row.email_confirmed,
                created: row.created,
                version: row.data_version,
            }))
    }

    #[instrument(skip(self))]
    async fn delete_link(
        &mut self,
        user_id: Uuid,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<()>, IdentityError> {
        let count = self
            .stmts_external_links
            .delete_link
            .execute(&self.client, &user_id, &provider, &provider_id)
            .await
            .map_err(DBError::from)?;

        if count == 1 {
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }
}
