use chrono::{DateTime, Utc};
use futures::{future::BoxFuture, FutureExt};
use sqlx::{AnyPool, FromRow};
use sqlx_interpolation::{sql_expr, types, DBKind};
use uuid::Uuid;

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

            let row: Option<(i32, Uuid, DateTime<Utc>)> = sql_expr!(
                self.db_kind(),
                "INSERT INTO identities (kind, id, creation)" 
                + "VALUES(0, ${id}, ${expr::Now})" 
                + "ON CONFLICT DO NOTHING"
                + "RETURNING (id, kind, creation)"
            )
            .to_query()
            .fetch_one(&self.pool)
            .await?;

            if let Some(row) = row {
                assert_eq!(row.0, 0);
                assert_eq!(row.1, id);
                log::info!("user created: {:?}, {:?}, {:?}", row.0, row.1, row.2 );
                    return Ok(Identity{
                        id,
                        kind: IdentityKind::User,
                        creation: row.2,
                    })
            }
        }

        Err(AppError::DBRetryLimitReached)
    }

    async fn find_by_id(&self, id: uuid) -> Result<Option<Identity>, AppError> {
        todo!()
    }
}
