use crate::repositories::{Identity, IdentityBuildError, IdentityError, IdentityKind};
use chrono::{DateTime, Utc};
use shine_service::{
    pg_query,
    service::{PGClient, PGConnection, PGErrorChecks as _, PGRawConnection},
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ExternalUserInfo {
    pub provider: String,
    pub provider_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ExternalLink {
    pub user_id: Uuid,
    pub provider: String,
    pub provider_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub linked_at: DateTime<Utc>,
}

pg_query!( InsertExternalLogin =>
    in = user_id: Uuid, provider: &str, provider_id: &str, name: Option<&str>, email: Option<&str>;
    out = linked: DateTime<Utc>;
    sql = r#"
        INSERT INTO external_logins (user_id, provider, provider_id, name, email, linked) 
            VALUES ($1, $2, $3, $4, $5, now())
        RETURNING linked
    "#
);

pg_query!( FindByProviderId =>
    in = provider: &str, provider_id: &str;
    out = FindByLinkRow {
        user_id: Uuid,
        kind: IdentityKind,
        name: String,
        email: Option<String>,
        is_email_confirmed: bool,
        created: DateTime<Utc>,
        version: i32,
        provider: String,
        provider_id: String,
        external_name: Option<String>,
        external_email: Option<String>,
        linked: DateTime<Utc>
    };
    sql = r#"
        SELECT i.user_id, i.kind, i.name, i.email, i.email_confirmed, i.created, i.data_version,
            e.provider, e.provider_id, e.name, e.email, e.linked
            FROM external_logins e, identities i
            WHERE e.user_id = i.user_id
                AND e.provider = $1
                AND e.provider_id = $2
    "#
);

pg_query!( ListByUserId =>
    in = user_id: Uuid;
    out = ListByUserIdRow {
        user_id: Uuid,
        provider: String,
        provider_id: String,
        name: Option<String>,
        email: Option<String>,
        linked_at: DateTime<Utc>
    };
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

pub struct ExternalLinksStatements {
    insert: InsertExternalLogin,
    find_by_provider_id: FindByProviderId,
    list_by_user_id: ListByUserId,
    delete_link: DeleteLink,
}

impl ExternalLinksStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            insert: InsertExternalLogin::new(client).await?,
            find_by_provider_id: FindByProviderId::new(client).await?,
            list_by_user_id: ListByUserId::new(client).await?,
            delete_link: DeleteLink::new(client).await?,
        })
    }
}

/// External Links Data Access Object.
pub struct ExternalLinks<'a, T>
where
    T: PGRawConnection,
{
    client: &'a PGConnection<T>,
    stmts_external_links: &'a ExternalLinksStatements,
}

impl<'a, T> ExternalLinks<'a, T>
where
    T: PGRawConnection,
{
    pub fn new(client: &'a PGConnection<T>, stmts_external_links: &'a ExternalLinksStatements) -> Self {
        Self {
            client,
            stmts_external_links,
        }
    }

    pub async fn link_user(&mut self, user_id: Uuid, external_user: &ExternalUserInfo) -> Result<(), IdentityError> {
        match self
            .stmts_external_links
            .insert
            .query_one(
                self.client,
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

    pub async fn find_all(&mut self, user_id: Uuid) -> Result<Vec<ExternalLink>, IdentityError> {
        let links = self
            .stmts_external_links
            .list_by_user_id
            .query(self.client, &user_id)
            .await?
            .into_iter()
            .map(|row| ExternalLink {
                user_id,
                provider: row.provider,
                provider_id: row.provider_id,
                name: row.name,
                email: row.email,
                linked_at: row.linked_at,
            })
            .collect();

        Ok(links)
    }

    pub async fn find_by_external_link(
        &mut self,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<Identity>, IdentityError> {
        Ok(self
            .stmts_external_links
            .find_by_provider_id
            .query_opt(self.client, &provider, &provider_id)
            .await?
            .map(|row| Identity {
                id: row.user_id,
                kind: row.kind,
                name: row.name,
                email: row.email,
                is_email_confirmed: row.is_email_confirmed,
                created: row.created,
                version: row.version,
            }))
    }

    pub async fn delete_link(
        &mut self,
        user_id: Uuid,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<()>, IdentityError> {
        let count = self
            .stmts_external_links
            .delete_link
            .execute(self.client, &user_id, &provider, &provider_id)
            .await?;

        if count == 1 {
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }
}
