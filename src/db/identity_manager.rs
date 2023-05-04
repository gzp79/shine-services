use crate::db::DBError;
use chrono::{DateTime, Utc};
use sqlx::{types::uuid::Uuid, AnyPool};
use sqlx_interpolation::{expr, sql_expr, DBKind};

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

#[derive(Debug)]
pub struct ExternalLogin {
    pub provider: String,
    pub provider_id: String,
}

#[derive(Clone)]
pub struct IdentityManager {
    pool: AnyPool,
}

impl IdentityManager {
    pub fn new(pool: AnyPool) -> Self {
        Self { pool }
    }

    pub fn db_kind(&self) -> DBKind {
        DBKind::from(self.pool.any_kind())
    }

    pub async fn create_user(
        &self,
        name: String,
        email: Option<String>,
        external_login: Option<ExternalLogin>,
    ) -> Result<Identity, DBError> {
        for _ in 0..10 {
            let id = Uuid::new_v4();
            let id_str = id.hyphenated().to_string();
            //let email = email.map(|e| e.normalize_email());

            let mut tx = self.pool.begin().await?;

            let user_row = sql_expr!(
                self.db_kind(),
                "INSERT INTO identities (user_id, kind, created, name, email)"
                    + "VALUES(uuid(${&id_str}), 0, ${expr::Now}, ${&name}, ${&email})"
                    + "ON CONFLICT DO NOTHING"
                    + "RETURNING text(user_id), kind, created"
            )
            .to_query_as::<_, (String, i32, DateTime<Utc>)>()
            .fetch_optional(&mut tx)
            .await?;

            if let Some(row) = user_row {
                log::info!("row: {:?}", row);

                if let Some(external_login) = external_login {
                    let link_response = sql_expr!(
                        self.db_kind(),
                            "INSERT INTO external_logins (user_id, provider, provider_id, linked)"
                            + "VALUES(${&id_str}, ${&external_login.provider}, ${&external_login.provider_id}, ${expr::Now})"
                            + "ON CONFLICT DO NOTHING"
                            + "RETURNING 'ok'"
                    )
                    .to_query_as::<_, (String, )>()
                    .fetch_optional(&mut tx)
                    .await?;

                    // check if link could be added
                    if link_response.unwrap_or_default().0 != "ok" {
                        tx.rollback().await?;
                        return Err(DBError::Conflict);
                    }
                }

                assert_eq!(row.0, id_str);
                assert_eq!(row.1, 0);
                log::info!("user created: {:?}, {:?}, {:?}", row.0, row.1, row.2);
                tx.commit().await?;
                return Ok(Identity {
                    id,
                    kind: IdentityKind::User,
                    creation: row.2,
                });
            } else {
                // retry, user_id had a conflict - very unlikely, but it's safer this way.
                tx.rollback().await?;
            }
        }

        Err(DBError::RetryLimitReached)
    }

    pub async fn find_user_by_id(&self, id: Uuid) -> Result<Option<Identity>, DBError> {
        todo!()
    }

    pub async fn link_user(&self, user_id: Uuid, external_login: ExternalLogin) -> Result<(), DBError> {
        todo!()
    }

    pub async fn unlink_user(&self, user_id: Uuid, provider: String) -> Result<(), DBError> {
        todo!()
    }

    pub async fn get_linked_providers(&self, user_id: Uuid) -> Result<Vec<ExternalLogin>, DBError> {
        todo!()
    }
}
