use chrono::{DateTime, Utc};
use sqlx::{types::uuid::Uuid, AnyPool};
use sqlx_interpolation::{expr, sql_expr, DBKind};
use crate::db::DBError;

#[derive(Debug, Clone, Copy)]
pub enum IdentityKind {
    User,
}

#[derive(Debug)]
pub struct Identity {
    pub id: Uuid,
    pub kind: IdentityKind,
    pub creation: DateTime<Utc>,
}

#[derive(Clone)]
pub struct IdentityManager {
    pool: AnyPool,
}

#[derive(Debug)]
pub struct ExternalLogin {
    pub provider: String,
    pub id_token: String,    
}

impl IdentityManager {
    pub async fn new(pool: AnyPool) -> Self {
        Self { pool }
    }

    pub fn db_kind(&self) -> DBKind {
        DBKind::from(self.pool.any_kind())
    }

    pub async fn create_user(&self, name: String, email: Option<String>, external_login: Option<ExternalLogin>) -> Result<Identity, DBError> {
        for _ in 0..10 {
            let id = Uuid::new_v4();
            let id_str = id.hyphenated().to_string();
            //let email = email.map(|e| e.normalize_email());

            //todo: create transaction

            let row = sql_expr!(
                self.db_kind(),
                "INSERT INTO identities (kind, id, creation, name, email)"
                    + "VALUES(0, ${&id_str}, ${expr::Now}, ${&name}, ${&email})"
                    + "ON CONFLICT DO NOTHING"
                    + "RETURNING (id, kind, creation)"
            )
            .to_query_as::<_, (i32, String, DateTime<Utc>)>()
            .fetch_optional(&self.pool)
            .await?;

            //todo: store external login

            if let Some(row) = row {
                assert_eq!(row.0, 0);
                assert_eq!(row.1, id_str);
                log::info!("user created: {:?}, {:?}, {:?}", row.0, row.1, row.2);
                return Ok(Identity {
                    id,
                    kind: IdentityKind::User,
                    creation: row.2,
                });
            }
        }

        Err(DBError::RetryLimitReached)
    }
    
    pub async fn find_user_by_id(&self, id: Uuid) -> Result<Option<Identity>, DBError> {
        todo!()
    }

    pub async fn link_user(&self, user_id: Uuid, external_login: ExternalLogin) -> Result<(), DBError>{
        todo!()
    }

    pub async fn unlink_user(&self, user_id: Uuid, provider: String) -> Result<(), DBError> {
        todo!()
    }

    pub async fn get_linked_providers(&self, user_id: Uuid) -> Result<Vec<ExternalLogin>, DBError> {
        todo!()
    }
    
}
