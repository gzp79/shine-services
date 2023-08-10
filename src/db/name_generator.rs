use crate::db::{DBError, DBPool};
use harsh::Harsh;
use serde::{Deserialize, Serialize};
use shine_service::{pg_query, service::PGConnectionPool, utils::Optimus};
use std::sync::Arc;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum NameGeneratorError {
    #[error(transparent)]
    DBError(#[from] DBError),
    #[error("Base name generator error: {0}")]
    BaseGenerator(String),
    #[error("Id encoder error: {0}")]
    IdEncoder(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "baseGenerator")]
pub enum BaseGeneratorConfig {
    #[serde(rename_all = "camelCase")]
    Fixed { base_name: String },
}

impl BaseGeneratorConfig {
    fn create_generator(&self) -> Result<Box<dyn BaseGenerator>, NameGeneratorError> {
        match self {
            BaseGeneratorConfig::Fixed { base_name } => {
                if !(3..10).contains(&base_name.len()) {
                    Err(NameGeneratorError::BaseGenerator(
                        "Base name length should be in the range [3,10)".into(),
                    ))
                } else {
                    Ok(Box::new(base_name.clone()))
                }
            }
        }
    }
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
    fn create_encoder(&self) -> Result<Box<dyn IdEncoder>, NameGeneratorError> {
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
                    .map_err(|err| NameGeneratorError::IdEncoder(format!("{err}")))?;
                Ok(Box::new(harsh))
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameGeneratorConfig {
    #[serde(flatten)]
    base_generator: BaseGeneratorConfig,
    #[serde(flatten)]
    id_encoder: IdEncoderConfig,
}

/// Trait to generate some base name use as the prefix
trait BaseGenerator: 'static + Send + Sync {
    fn generate(&self) -> String;
}

impl BaseGenerator for String {
    fn generate(&self) -> String {
        self.clone()
    }
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
    out = id: i32;
    sql = r#"
        SELECT nextval('user_id_counter')
    "#
);

struct Inner {
    postgres: PGConnectionPool,
    stmt_next_id: GetNextId,
    base: Box<dyn BaseGenerator>,
    id_encoder: Box<dyn IdEncoder>,
}

#[derive(Clone)]
pub struct NameGenerator(Arc<Inner>);

impl NameGenerator {
    pub async fn new(config: &NameGeneratorConfig, pool: &DBPool) -> Result<Self, NameGeneratorError> {
        let client = pool.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        let stmt_next_id = GetNextId::new(&client).await.map_err(DBError::from)?;

        Ok(Self(Arc::new(Inner {
            postgres: pool.postgres.clone(),
            stmt_next_id,
            base: config.base_generator.create_generator()?,
            id_encoder: config.id_encoder.create_encoder()?,
        })))
    }

    pub async fn generate_name(&self) -> Result<String, NameGeneratorError> {
        // some alternatives and sources:
        // - <https://datatracker.ietf.org/doc/html/rfc1751>
        // - <https://github.com/archer884/harsh>
        // - <https://github.com/pjebs/optimus-go>

        let inner = &*self.0;

        let client = inner.postgres.get().await.map_err(DBError::PostgresPoolError)?;

        let prefix = inner.base.generate();
        let suffix = {
            let id = inner.stmt_next_id.query_one(&client).await.map_err(DBError::from)?;
            inner.id_encoder.encode(id as u64)
        };

        Ok(format!("{}_{}", prefix, suffix))
    }
}
