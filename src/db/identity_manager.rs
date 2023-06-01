use crate::db::{DBError, DBErrorChecks, DBPool};
use chrono::{DateTime, Utc};
use tokio_postgres::error::SqlState;
use uuid::Uuid;

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
    pool: DBPool,
}

impl IdentityManager {
    pub fn new(pool: DBPool) -> Self {
        Self { pool }
    }

    pub async fn create_user(
        &self,
        name: String,
        email: Option<String>,
        external_login: Option<&ExternalLogin>,
    ) -> Result<Identity, DBError> {
        for _ in 0..10 {
            //let id = Uuid::new_v4();
            let id = Uuid::parse_str("411da543-051c-4596-9184-412a9a2833a4").unwrap();
            //let email = email.map(|e| e.normalize_email());

            let mut client = self.pool.get().await?;

            let transaction = client.transaction().await?;

            let stmt = transaction
                .prepare(
                    "INSERT INTO identities (user_id, kind, created, name, email) VALUES ($1, 0, now(), $2, $3)
                            RETURNING created",
                )
                .await?;

            let row: DateTime<Utc> = match transaction.query_one(&stmt, &[&id, &name, &email]).await {
                Ok(row) => row.get(0),
                Err(err) if err.is_constraint("identities", "idx_name") => {
                    // conflict - name already taken, generate a new
                    transaction.rollback().await?;
                    continue;
                }
                Err(err) if err.is_constraint("identities", "identities_pkey") => {
                    // conflict - uuid is not unique
                    transaction.rollback().await?;
                    continue;
                }
                Err(err) => {
                    return Err(err.into());
                }
            };

            if let Some(external_login) = external_login {
                let stmt = transaction
                .prepare("INSERT INTO external_logins (user_id, provider, provider_id, linked) VALUES ($1, $2, $3, now())")
                .await?;
                transaction
                    .execute(&stmt, &[&id, &external_login.provider, &external_login.provider_id])
                    .await?;
            }
            log::info!("4");

            match transaction.commit().await {
                Ok(_) => {
                    return Ok(Identity {
                        id,
                        kind: IdentityKind::User,
                        creation: row.get(0),
                    })
                }
                Err(err) => {
                    log::info!("5");
                    if let Some(err) = err.as_db_error() {
                        if &SqlState::UNIQUE_VIOLATION == err.code() {
                            log::info!("{:?}", err);
                            let detail = err.message().to_lowercase();
                            if err.table() == Some("identities") && detail.contains("idx_name") {
                                // conflict - name already taken, generate a new
                                continue;
                            } else if err.table() == Some("identities") && detail.contains("identities_pkey") {
                                // conflict - uuid is not unique
                                continue;
                            }
                        }
                    }
                    return Err(err.into());
                }
            }
        }

        Err(DBError::RetryLimitReached)
    }

    pub async fn find_user_by_id(&self, id: Uuid) -> Result<Option<Identity>, DBError> {
        todo!()
    }

    pub async fn find_user_by_email(&self, email: String) -> Result<Option<Identity>, DBError> {
        todo!()
    }

    pub async fn find_user_by_link(&self, external_login: &ExternalLogin) -> Result<Option<Identity>, DBError> {
        /*let identity = sql_expr!(
            self.db_kind(),
            "SELECT identities.user_id, kind, created from external_logins, identities"
                + "WHERE external_logins.user_id = identities.user_id"
                + " AND external_logins.provider = ${&external_login.provider}"
                + " AND external_logins.provider_id = ${&external_login.provider_id}"
        )
        .to_query_as::<_, (DBUuid, i32, DBDateTime)>()
        .fetch_optional(&self.pool)
        .await?;

        if let Some((user_id, kind, creation)) = identity {
            Ok(Some(Identity {
                id: user_id.0,
                kind: IdentityKind::try_from(kind)?,
                creation: creation.0,
            }))
        } else {
            Ok(None)
        }*/
        todo!()
    }

    pub async fn link_user(&self, user_id: Uuid, external_login: &ExternalLogin) -> Result<(), DBError> {
        /*let id_str = user_id.hyphenated().to_string();
        let link_response = sql_expr!(
            self.db_kind(),
            "INSERT INTO external_logins (user_id, provider, provider_id, linked)"
                + "VALUES(uuid(${&id_str}), ${&external_login.provider}, ${&external_login.provider_id}, ${expr::Now})"
                + "ON CONFLICT DO NOTHING"
                + "RETURNING 'ok'"
        )
        .to_query_as::<_, (String,)>()
        .fetch_optional(&self.pool)
        .await?;

        // check if link could be added
        if link_response.unwrap_or_default().0 == "ok" {
            Ok(())
        } else {
            Err(DBError::Conflict)
        }*/
        todo!()
    }

    pub async fn unlink_user(&self, user_id: Uuid, provider: String) -> Result<(), DBError> {
        todo!()
    }

    pub async fn get_linked_providers(&self, user_id: Uuid) -> Result<Vec<ExternalLogin>, DBError> {
        todo!()
    }
}
