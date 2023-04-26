use chrono::{DateTime, Utc};
use sqlx::{types::uuid::Uuid, AnyPool};
use sqlx_interpolation::{expr, sql_expr, DBKind};

use crate::app_error::AppError;

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

impl IdentityManager {
    pub async fn new(pool: AnyPool) -> Self {
        Self { pool }
    }

    pub fn db_kind(&self) -> DBKind {
        DBKind::from(self.pool.any_kind())
    }

    async fn create_user(&self) -> Result<Identity, AppError> {
        for _ in 0..10 {
            let id = Uuid::new_v4();
            let id_str = id.hyphenated().to_string();

            let row = sql_expr!(
                self.db_kind(),
                "INSERT INTO identities (kind, id, creation)"
                    + "VALUES(0, ${&id_str}, ${expr::Now})"
                    + "ON CONFLICT DO NOTHING"
                    + "RETURNING (id, kind, creation)"
            )
            .to_query_as::<_, (i32, String, DateTime<Utc>)>()
            .fetch_optional(&self.pool)
            .await?;

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

        Err(AppError::DBRetryLimitReached)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Identity>, AppError> {
        todo!()
    }
}
