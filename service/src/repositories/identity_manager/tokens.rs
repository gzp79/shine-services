use crate::repositories::{Identity, IdentityBuildError, IdentityError, IdentityKind};
use bytes::BytesMut;
use chrono::{DateTime, Duration, Utc};
use postgres_from_row::FromRow;
use ring::digest;
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::SiteInfo,
    pg_query,
    service::{
        ClientFingerprint, PGClient, PGConnection, PGConvertError, PGErrorChecks as _, PGRawConnection, ToPGType,
    },
};
use tokio_postgres::types::{accepts, to_sql_checked, FromSql, IsNull, ToSql, Type as PGType};
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TokenKind {
    SingleAccess,
    Persistent,
    Access,
}

impl ToSql for TokenKind {
    fn to_sql(&self, ty: &PGType, out: &mut BytesMut) -> Result<IsNull, PGConvertError> {
        let value = match self {
            TokenKind::SingleAccess => 1_i16,
            TokenKind::Persistent => 2_i16,
            TokenKind::Access => 3_i16,
        };
        value.to_sql(ty, out)
    }

    accepts!(INT2);
    to_sql_checked!();
}

impl<'a> FromSql<'a> for TokenKind {
    fn from_sql(ty: &PGType, raw: &[u8]) -> Result<TokenKind, PGConvertError> {
        let value = i16::from_sql(ty, raw)?;
        match value {
            1 => Ok(TokenKind::SingleAccess),
            2 => Ok(TokenKind::Persistent),
            3 => Ok(TokenKind::Access),
            _ => Err(PGConvertError::from("Invalid value for TokenKind")),
        }
    }

    accepts!(INT2);
}

impl ToPGType for TokenKind {
    const PG_TYPE: PGType = PGType::INT2;
}

#[derive(Debug)]
pub struct TokenInfo {
    pub user_id: Uuid,
    pub kind: TokenKind,
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub expire_at: DateTime<Utc>,
    pub is_expired: bool,
    pub fingerprint: Option<String>,
    pub agent: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

#[derive(FromRow)]
struct InsertTokenRow {
    created: DateTime<Utc>,
    expire: DateTime<Utc>,
}

pg_query!( InsertToken =>
    in = user_id: Uuid, token: &str,
        fingerprint: Option<&str>, kind: TokenKind,
        expire_s: i32,
        agent: &str, country: Option<&str>, region: Option<&str>, city: Option<&str>;
    out = InsertTokenRow;
    sql =  r#"
        INSERT INTO login_tokens (
                user_id, token, created, 
                fingerprint, kind,  
                expire, 
                agent, country, region, city)
            VALUES (
                $1, $2, now(), 
                $3, $4, 
                now() + $5 * interval '1 seconds',
                $6, $7, $8, $9)
        RETURNING created, expire
    "#
);

#[derive(FromRow)]
struct TokenRow {
    user_id: Uuid,
    token: String,
    created: DateTime<Utc>,
    expire: DateTime<Utc>,
    fingerprint: Option<String>,
    kind: TokenKind,
    is_expired: bool,
    agent: String,
    country: Option<String>,
    region: Option<String>,
    city: Option<String>,
}

pg_query!( FindByHashToken =>
    in = token: &str;
    out = TokenRow;
    sql = r#"
        SELECT t.user_id, t.token, t.created, t.expire, t.fingerprint, t.kind, t.expire < now() is_expired,
                t.agent, t.country, t.region, t.city
            FROM login_tokens t
            WHERE t.token = $1
    "#
);

pg_query!( ListByUser =>
    in = user_id: Uuid;
    out = TokenRow;
    sql = r#"
        SELECT t.user_id, t.token, t.created, t.expire, t.fingerprint, t.kind, t.expire < now() is_expired,
                t.agent, t.country, t.region, t.city
            FROM login_tokens t
            WHERE t.user_id = $1
    "#
);

pg_query!( DeleteToken =>
    in = token: &str, kind: TokenKind;
    sql = r#"
        DELETE FROM login_tokens WHERE token = $1 AND kind = $2
    "#
);

pg_query!( DeleteByUser =>
    in = user_id: Uuid, token: &str;
    sql = r#"
        DELETE FROM login_tokens WHERE user_id = $1 AND token = $2
    "#
);

pg_query!( DeleteAllByUser =>
    in = user_id: Uuid, kind: TokenKind;
    sql = r#"
        DELETE FROM login_tokens 
        WHERE user_id = $1 AND kind = $2
    "#
);

#[derive(FromRow)]
struct IdentityTokenRow {
    user_id: Uuid,
    kind: IdentityKind,
    name: String,
    email: Option<String>,
    email_confirmed: bool,
    created: DateTime<Utc>,
    data_version: i32,
    token_hash: String,
    token_created: DateTime<Utc>,
    token_expire: DateTime<Utc>,
    token_fingerprint: Option<String>,
    token_kind: TokenKind,
    token_is_expired: bool,
    token_agent: String,
    token_country: Option<String>,
    token_region: Option<String>,
    token_city: Option<String>,
}

// Test token for use. Compared to find it also returns the identity
pg_query!( TestToken =>
    in = token: &str, kind: TokenKind;
    out = IdentityTokenRow;
    sql = r#"
        SELECT i.user_id, i.kind, i.name, i.email, i.email_confirmed, i.created, i.data_version,
                t.token token_hash, 
                t.created token_created, 
                t.expire token_expire, 
                t.fingerprint token_fingerprint, 
                t.kind token_kind, 
                t.expire < now() token_is_expired,
                t.agent token_agent,
                t.country token_country,
                t.region token_region,
                t.city token_city
            FROM login_tokens t, identities i
            WHERE t.user_id = i.user_id
                AND t.token = $1
                AND t.kind = $2
    "#
);

// Test token for use. Compared to find it also returns the identity
pg_query!( TakeToken =>
    in = token: &str, kind: TokenKind;
    out = IdentityTokenRow;
    sql = r#"
    WITH t AS (
        DELETE FROM login_tokens lt
        WHERE lt.token = $1 AND lt.kind = $2
        RETURNING *        
    )
    SELECT i.user_id, i.kind, i.name, i.email, i.email_confirmed, i.created, i.data_version,
        t.token token_hash, 
        t.created token_created, 
        t.expire token_expire, 
        t.fingerprint token_fingerprint, 
        t.kind token_kind, 
        t.expire < now() token_is_expired,
        t.agent token_agent,
        t.country token_country,
        t.region token_region,
        t.city token_city
    FROM t, identities i
    WHERE t.user_id = i.user_id
    "#
);

pub struct TokensStatements {
    insert: InsertToken,
    find_by_hash: FindByHashToken,
    list_by_user: ListByUser,
    delete: DeleteToken,
    delete_by_user: DeleteByUser,
    delete_all_by_user: DeleteAllByUser,
    test: TestToken,
    take: TakeToken,
}

impl TokensStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            insert: InsertToken::new(client).await?,
            find_by_hash: FindByHashToken::new(client).await?,
            list_by_user: ListByUser::new(client).await?,
            delete: DeleteToken::new(client).await?,
            delete_by_user: DeleteByUser::new(client).await?,
            delete_all_by_user: DeleteAllByUser::new(client).await?,
            test: TestToken::new(client).await?,
            take: TakeToken::new(client).await?,
        })
    }
}

/// Handle tokens
pub struct Tokens<'a, T>
where
    T: PGRawConnection,
{
    client: &'a PGConnection<T>,
    stmts_tokens: &'a TokensStatements,
}

impl<'a, T> Tokens<'a, T>
where
    T: PGRawConnection,
{
    pub fn new(client: &'a PGConnection<T>, stmts_tokens: &'a TokensStatements) -> Self {
        Self { client, stmts_tokens }
    }

    #[instrument(skip(self))]
    pub async fn store_token(
        &mut self,
        user_id: Uuid,
        kind: TokenKind,
        token_hash: &str,
        time_to_live: &Duration,
        fingerprint: Option<&ClientFingerprint>,
        site_info: &SiteInfo,
    ) -> Result<TokenInfo, IdentityError> {
        let time_to_live = time_to_live.num_seconds() as i32;
        assert!(time_to_live > 2);
        let row = match self
            .stmts_tokens
            .insert
            .query_one(
                self.client,
                &user_id,
                &token_hash,
                &fingerprint.map(|f| f.as_str()),
                &kind,
                &time_to_live,
                &site_info.agent.as_str(),
                &site_info.country.as_deref(),
                &site_info.region.as_deref(),
                &site_info.city.as_deref(),
            )
            .await
        {
            Ok(row) => row,
            Err(err) if err.is_constraint("login_tokens", "idx_token") => {
                return Err(IdentityError::TokenConflict);
            }
            Err(err) if err.is_constraint("login_tokens", "chk_fingerprint") => {
                return Err(IdentityError::MissingFingerprint);
            }
            Err(err) => {
                return Err(IdentityError::DBError(err.into()));
            }
        };

        Ok(TokenInfo {
            user_id,
            kind,
            token_hash: token_hash.to_owned(),
            created_at: row.created,
            expire_at: row.expire,
            is_expired: false,
            fingerprint: fingerprint.map(|f| f.to_string()),
            agent: site_info.agent.clone(),
            country: site_info.country.clone(),
            region: site_info.region.clone(),
            city: site_info.city.clone(),
        })
    }

    #[instrument(skip(self))]
    pub async fn find_by_hash(&mut self, token_hash: &str) -> Result<Option<TokenInfo>, IdentityError> {
        Ok(self
            .stmts_tokens
            .find_by_hash
            .query_opt(self.client, &token_hash)
            .await?
            .map(|row| TokenInfo {
                user_id: row.user_id,
                kind: row.kind,
                token_hash: row.token,
                created_at: row.created,
                expire_at: row.expire,
                is_expired: row.is_expired,
                fingerprint: row.fingerprint,
                agent: row.agent,
                country: row.country,
                region: row.region,
                city: row.city,
            }))
    }

    #[instrument(skip(self))]
    pub async fn find_by_user(&mut self, user_id: &Uuid) -> Result<Vec<TokenInfo>, IdentityError> {
        Ok(self
            .stmts_tokens
            .list_by_user
            .query(self.client, user_id)
            .await?
            .into_iter()
            .map(|row| TokenInfo {
                user_id: row.user_id,
                kind: row.kind,
                token_hash: row.token,
                created_at: row.created,
                expire_at: row.expire,
                is_expired: row.is_expired,
                fingerprint: row.fingerprint,
                agent: row.agent,
                country: row.country,
                region: row.region,
                city: row.city,
            })
            .collect())
    }

    #[instrument(skip(self))]
    pub async fn delete_token(&mut self, kind: TokenKind, token_hash: &str) -> Result<Option<()>, IdentityError> {
        let count = self
            .stmts_tokens
            .delete
            .execute(self.client, &token_hash, &kind)
            .await?;
        if count == 1 {
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }

    #[instrument(skip(self))]
    pub async fn delete_by_user(&mut self, user_id: Uuid, token_hash: &str) -> Result<Option<()>, IdentityError> {
        let count = self
            .stmts_tokens
            .delete_by_user
            .execute(self.client, &user_id, &token_hash)
            .await?;
        if count == 1 {
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }

    #[instrument(skip(self))]
    pub async fn delete_all_by_user(&mut self, user_id: Uuid, kinds: &[TokenKind]) -> Result<(), IdentityError> {
        for kind in kinds {
            self.stmts_tokens
                .delete_all_by_user
                .execute(self.client, &user_id, kind)
                .await?;
        }
        Ok(())
    }

    /// Test the presence of a token and return the identity if found.
    #[instrument(skip(self))]
    pub async fn test_token(
        &mut self,
        kind: TokenKind,
        token_hash: &str,
    ) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let row = self
            .stmts_tokens
            .test
            .query_opt(self.client, &token_hash, &kind)
            .await?;
        Ok(row.map(|row| {
            let token = TokenInfo {
                user_id: row.user_id,
                kind: row.token_kind,
                token_hash: row.token_hash,
                created_at: row.token_created,
                expire_at: row.token_expire,
                is_expired: row.token_is_expired,
                fingerprint: row.token_fingerprint,
                agent: row.token_agent,
                country: row.token_country,
                region: row.token_region,
                city: row.token_city,
            };
            let identity = Identity {
                id: row.user_id,
                kind: row.kind,
                name: row.name,
                email: row.email,
                is_email_confirmed: row.email_confirmed,
                created: row.created,
                version: row.data_version,
            };
            (identity, token)
        }))
    }

    /// Take a token and return the identity if found.
    /// The token is deleted from the database.
    #[instrument(skip(self))]
    pub async fn take_token(
        &mut self,
        kind: TokenKind,
        token_hash: &str,
    ) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let row = self
            .stmts_tokens
            .take
            .query_opt(self.client, &token_hash, &kind)
            .await?;
        Ok(row.map(|row| {
            let token = TokenInfo {
                user_id: row.user_id,
                kind: row.token_kind,
                token_hash: row.token_hash,
                created_at: row.token_created,
                expire_at: row.token_expire,
                is_expired: row.token_is_expired,
                fingerprint: row.token_fingerprint,
                agent: row.token_agent,
                country: row.token_country,
                region: row.token_region,
                city: row.token_city,
            };
            let identity = Identity {
                id: row.user_id,
                kind: row.kind,
                name: row.name,
                email: row.email,
                is_email_confirmed: row.email_confirmed,
                created: row.created,
                version: row.data_version,
            };
            (identity, token)
        }))
    }
}

/// Generate a (crypto) hashed version of a token to protect data in rest.
pub fn hash_token(token: &str) -> String {
    // there is no need for a complex hash as key has a big entropy already
    // and it'd be too expensive to invert the hashing.
    let hash = digest::digest(&digest::SHA256, token.as_bytes());
    let hash = hex::encode(hash);
    log::debug!("Hashing token: {token:?} -> [{hash}]");
    hash
}
