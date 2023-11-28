use crate::repositories::{Identity, IdentityBuildError, IdentityError, IdentityKind};
use bytes::BytesMut;
use chrono::{DateTime, Duration, Utc};
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

pg_query!( InsertToken =>
    in = user_id: Uuid, token: &str,
        fingerprint: Option<&str>, kind: TokenKind,
        expire_s: i32,
        agent: &str, country: Option<&str>, region: Option<&str>, city: Option<&str>;
    out = InsertTokenRow{
        created: DateTime<Utc>,
        expire: DateTime<Utc>
    };
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

pg_query!( FindToken =>
    in = token: &str;
    out = FindByTokenRow{
        user_id: Uuid,
        kind: IdentityKind,
        name: String,
        email: Option<String>,
        is_email_confirmed: bool,
        created: DateTime<Utc>,
        version: i32,
        token: String,
        token_created: DateTime<Utc>,
        token_expire: DateTime<Utc>,
        token_fingerprint: Option<String>,
        token_kind: TokenKind,
        token_is_expired: bool,
        token_agent: String,
        token_country: Option<String>,
        token_region: Option<String>,
        token_city: Option<String>
    };
    sql = r#"
        SELECT i.user_id, i.kind, i.name, i.email, i.email_confirmed, i.created, i.data_version,
                t.token, t.created, t.expire, t.fingerprint, t.kind, t.expire < now() is_expired,
                t.agent, t.country, t.region, t.city
            FROM login_tokens t, identities i
            WHERE t.user_id = i.user_id
                AND t.token = $1
    "#
);

pg_query!( ListTokens =>
    in = user_id: Uuid;
    out = TokenRow{
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
        city: Option<String>
    };
    sql = r#"
        SELECT t.user_id, t.token, t.created, t.expire, t.fingerprint, t.kind, t.expire < now() is_expired,
                t.agent, t.country, t.region, t.city
            FROM login_tokens t
            WHERE t.user_id = $1
    "#
);

pg_query!( DeleteToken =>
    in = user_id: Uuid, token: &str;
    sql = r#"
        DELETE FROM login_tokens WHERE user_id = $1 AND token = $2
    "#
);

pg_query!( DeleteAllTokens =>
    in = user_id: Uuid, kind: TokenKind;
    sql = r#"
        DELETE FROM login_tokens 
        WHERE user_id = $1 AND kind = $2
    "#
);

pub struct TokensStatements {
    insert: InsertToken,
    find: FindToken,
    list: ListTokens,
    delete: DeleteToken,
    delete_all: DeleteAllTokens,
}

impl TokensStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            insert: InsertToken::new(client).await?,
            find: FindToken::new(client).await?,
            list: ListTokens::new(client).await?,
            delete: DeleteToken::new(client).await?,
            delete_all: DeleteAllTokens::new(client).await?,
        })
    }
}

/// Tokens Data Access Object.
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

    pub async fn find_token(&mut self, token_hash: &str) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let row = self.stmts_tokens.find.query_opt(self.client, &token_hash).await?;
        Ok(row.map(|row| {
            let token = TokenInfo {
                user_id: row.user_id,
                kind: row.token_kind,
                token_hash: row.token,
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
                is_email_confirmed: row.is_email_confirmed,
                created: row.created,
                version: row.version,
            };
            (identity, token)
        }))
    }

    pub async fn find_by_user(&mut self, user_id: &Uuid) -> Result<Vec<TokenInfo>, IdentityError> {
        Ok(self
            .stmts_tokens
            .list
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

    pub async fn delete_token(&mut self, user_id: Uuid, token_hash: &str) -> Result<Option<()>, IdentityError> {
        let count = self
            .stmts_tokens
            .delete
            .execute(self.client, &user_id, &token_hash)
            .await?;
        if count == 1 {
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }

    pub async fn delete_all_tokens(&mut self, user_id: Uuid, kinds: &[TokenKind]) -> Result<(), IdentityError> {
        for kind in kinds {
            self.stmts_tokens
                .delete_all
                .execute(self.client, &user_id, kind)
                .await?;
        }
        Ok(())
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
