use crate::repositories::DBError;
use harsh::Harsh;
use serde::{Deserialize, Serialize};
use shine_service::{pg_query, service::PGConnectionPool, utils::Optimus};
use std::sync::Arc;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum AutoNameBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
    #[error("Base name generator error: {0}")]
    NameGenerator(String),
    #[error("Id encoder error: {0}")]
    IdEncoder(String),
}

#[derive(Debug, ThisError)]
pub enum AutoNameError {
    #[error(transparent)]
    DBError(#[from] DBError),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "idEncoder")]
pub enum IdEncoderConfig {
    #[serde(rename_all = "camelCase")]
    Optimus { prime: u64, random: u64 },

    #[serde(rename_all = "camelCase")]
    Harsh { salt: String },
}

impl IdEncoderConfig {
    fn create_encoder(&self) -> Result<Box<dyn IdEncoder>, AutoNameBuildError> {
        match self {
            IdEncoderConfig::Optimus { prime, random } => Ok(Box::new(Optimus::new(*prime, *random))),
            IdEncoderConfig::Harsh { salt } => {
                const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz1234567890";
                const SEPARATORS: &[u8] = b"cfhistu";

                let harsh = Harsh::builder()
                    .salt(salt.as_bytes())
                    .length(6)
                    .alphabet(ALPHABET)
                    .separators(SEPARATORS)
                    .build()
                    .map_err(|err| AutoNameBuildError::IdEncoder(format!("{err}")))?;
                Ok(Box::new(harsh))
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoNameConfig {
    base_name: String,
    #[serde(flatten)]
    id_encoder: IdEncoderConfig,
}

/// Trait to generate some obfuscated id for a sequence number
trait IdEncoder: 'static + Send + Sync {
    fn encode(&self, id: u64) -> String;
}

impl IdEncoder for Optimus {
    fn encode(&self, id: u64) -> String {
        Optimus::encode(self, id).to_string()
    }
}

impl IdEncoder for Harsh {
    fn encode(&self, id: u64) -> String {
        Harsh::encode(self, &[id])
    }
}

pg_query!( GetNextId =>
    in = ;
    out = id: i64;
    sql = r#"
        SELECT nextval('user_id_counter') as id
    "#
);

struct Inner {
    postgres: PGConnectionPool,
    stmt_next_id: GetNextId,
    base_name: String,
    id_encoder: Box<dyn IdEncoder>,
}

#[derive(Clone)]
pub struct AutoNameManager(Arc<Inner>);

impl AutoNameManager {
    pub async fn new(config: &AutoNameConfig, postgres: &PGConnectionPool) -> Result<Self, AutoNameBuildError> {
        let client = postgres.get().await.map_err(DBError::PGPoolError)?;
        let stmt_next_id = GetNextId::new(&client).await.map_err(DBError::from)?;

        if !(3..10).contains(&config.base_name.len()) {
            return Err(AutoNameBuildError::NameGenerator(
                "Base name length should be in the range [3,10)".into(),
            ));
        }

        Ok(Self(Arc::new(Inner {
            postgres: postgres.clone(),
            stmt_next_id,
            base_name: config.base_name.clone(),
            id_encoder: config.id_encoder.create_encoder()?,
        })))
    }

    pub async fn generate_name(&self) -> Result<String, AutoNameError> {
        // some alternatives and sources:
        // - <https://datatracker.ietf.org/doc/html/rfc1751>
        // - <https://github.com/archer884/harsh>
        // - <https://github.com/pjebs/optimus-go>

        let inner = &*self.0;

        let client = inner.postgres.get().await.map_err(DBError::PGPoolError)?;

        let prefix = &inner.base_name;
        let suffix = {
            let id = inner.stmt_next_id.query_one(&client).await.map_err(DBError::from)?;
            inner.id_encoder.encode(id as u64)
        };

        Ok(format!("{}_{}", prefix, suffix))
    }
}
