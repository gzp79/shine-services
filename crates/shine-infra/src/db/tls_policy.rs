use anyhow::{anyhow, Error as AnyError};
use tokio_postgres::config::SslMode;

/// The stage name every `server_config.<stage>.json` production deployment uses.
const PROD_STAGE: &str = "prod";

/// Fails if `stage` is production and `cns`'s `sslmode` is not `require`.
pub fn check_postgres_tls(stage: &str, cns: &str) -> Result<(), AnyError> {
    if stage != PROD_STAGE {
        return Ok(());
    }
    let config: tokio_postgres::Config = cns.parse()?;
    match config.get_ssl_mode() {
        SslMode::Require => Ok(()),
        other => Err(anyhow!(
            "Production Postgres connections must set sslmode=require, found {other:?}"
        )),
    }
}

/// Fails if `stage` is production and `cns` is not a `rediss://` (TLS) connection string.
pub fn check_redis_tls(stage: &str, cns: &str) -> Result<(), AnyError> {
    if stage != PROD_STAGE {
        return Ok(());
    }
    if cns.starts_with("rediss://") {
        Ok(())
    } else {
        Err(anyhow!("Production Redis connections must use rediss://, not redis://"))
    }
}
