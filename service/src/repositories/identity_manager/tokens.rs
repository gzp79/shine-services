use crate::repositories::{Identity, IdentityBuildError, IdentityError, IdentityKind, SiteInfo};
use bytes::BytesMut;
use chrono::{DateTime, Duration, Utc};
use shine_service::{
    pg_query,
    service::{
        ClientFingerprint, PGClient, PGConnection, PGConvertError, PGErrorChecks as _, PGRawConnection, ToPGType,
    },
};
use tokio_postgres::types::{accepts, to_sql_checked, FromSql, IsNull, ToSql, Type as PGType};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum TokenKind {
    SingleAccess,
    Persistent,
    AutoRenewal,
}

impl ToSql for TokenKind {
    fn to_sql(&self, ty: &PGType, out: &mut BytesMut) -> Result<IsNull, PGConvertError> {
        let value = match self {
            TokenKind::SingleAccess => 1_i16,
            TokenKind::Persistent => 2_i16,
            TokenKind::AutoRenewal => 3_i16,
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
            3 => Ok(TokenKind::AutoRenewal),
            _ => Err(PGConvertError::from("Invalid value for TokenKind")),
        }
    }

    accepts!(INT2);
}

impl ToPGType for TokenKind {
    const PG_TYPE: PGType = PGType::INT2;
}

#[derive(Debug)]
pub struct CurrentToken {
    pub user_id: Uuid,
    pub token: String,
    pub fingerprint: Option<String>,
    pub expire: DateTime<Utc>,
    pub is_expired: bool,
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
                $5, $6, $7, $8)
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
        token_is_expired: bool
    };
    sql = r#"
        SELECT i.user_id, i.kind, i.name, i.email, i.email_confirmed, i.created, i.data_version,
            t.token, t.created, t.expire, t.fingerprint, t.kind, t.expire < now() is_expired
            FROM login_tokens t, identities i
            WHERE t.user_id = i.user_id
                AND t.token = $1
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
    delete: DeleteToken,
    delete_all: DeleteAllTokens,
}

impl TokensStatements {
    pub async fn new(client: &PGClient) -> Result<Self, IdentityBuildError> {
        Ok(Self {
            insert: InsertToken::new(client).await?,
            find: FindToken::new(client).await?,
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

    pub async fn create_token(
        &mut self,
        user_id: Uuid,
        token: &str,
        duration: &Duration,
        fingerprint: Option<&ClientFingerprint>,
        site_info: &SiteInfo,
        kind: TokenKind,
    ) -> Result<CurrentToken, IdentityError> {
        let duration = duration.num_seconds() as i32;
        assert!(duration > 2);
        let row = match self
            .stmts_tokens
            .insert
            .query_one(
                self.client,
                &user_id,
                &token,
                &fingerprint.map(|f| f.as_str()),
                &kind,
                &duration,
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
            Err(err) => {
                return Err(IdentityError::DBError(err.into()));
            }
        };

        Ok(CurrentToken {
            user_id,
            token: token.to_owned(),
            expire: row.expire,
            fingerprint: fingerprint.map(|f| f.to_string()),
            is_expired: false,
        })
    }

    pub async fn find_token(&mut self, token: &str) -> Result<Option<(Identity, CurrentToken)>, IdentityError> {
        let row = self.stmts_tokens.find.query_opt(self.client, &token).await?;
        Ok(row.map(|row| {
            let token = CurrentToken {
                user_id: row.user_id,
                token: row.token,
                expire: row.token_expire,
                fingerprint: row.token_fingerprint,
                is_expired: row.token_is_expired,
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

    pub async fn update_token(&mut self, token: &str, duration: &Duration) -> Result<CurrentToken, IdentityError> {
        // issue#11:
        // - update expiration
        // - update last use

        let duration = duration.num_seconds() as i32;
        assert!(duration > 0);
        //issue#11:
        //  delete token where kind is SingleAccess
        //  update expire date where type is renewal
        //  delete token where expired

        // workaround while update is not implemented
        Ok(self.find_token(token).await?.ok_or(IdentityError::TokenConflict)?.1)
    }

    /*  pub async fn list_tokens(&self, user_id: Uuid) -> Result<Vec::<TokenInfo>, IdentityError> {
        let inner = &*self.0;
        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;
        inner.stmt_delete_token.execute(&client, &user_id, &token).await?;
        Ok(())
    }*/

    pub async fn delete_token(&mut self, user_id: Uuid, token: &str) -> Result<(), IdentityError> {
        self.stmts_tokens.delete.execute(self.client, &user_id, &token).await?;
        Ok(())
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
