use crate::db::{DBError, DBPool};
use chrono::{Utc, DateTime};
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
        /*for _ in 0..10 {
            let id = Uuid::new_v4();
            //let email = email.map(|e| e.normalize_email());

            let mut tx = self.pool.begin().await?;

            let user_row = sql_expr!(
                self.db_kind(),
                "INSERT INTO identities (user_id, kind, created, name, email)"
                    + "VALUES(${&id}, 0, ${expr::Now}, ${&name}, ${&email})"
                    + "ON CONFLICT DO NOTHING"
                    + "RETURNING user_id, kind, created"
            )
            .to_query_as::<_, (DBUuid, i32, DBDateTime)>()
            .fetch_optional(&mut tx)
            .await?;

            if let Some(row) = user_row {
                log::info!("row: {:?}", row);

                if let Some(external_login) = external_login {
                    let link_response = sql_expr!(
                        self.db_kind(),
                            "INSERT INTO external_logins (user_id, provider, provider_id, linked)"
                            + "VALUES(${&id}, ${&external_login.provider}, ${&external_login.provider_id}, ${expr::Now})"
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

                assert_eq!(row.0.0, id);
                assert_eq!(row.1, 0);
                log::info!("user created: {:?}, {:?}, {:?}", row.0, row.1, row.2);
                tx.commit().await?;
                return Ok(Identity {
                    id,
                    kind: IdentityKind::User,
                    creation: row.2.0,
                });
            } else {
                // retry, user_id had a conflict - very unlikely, but it's safer this way.
                tx.rollback().await?;
            }
        }

        Err(DBError::RetryLimitReached)*/
        todo!()
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
