use crate::db::redis::{RedisConnectionError, RedisConnectionPool, RedisRawError};
use bb8::RunError;
use redis::ErrorKind;

/// The stream scripts need exclusive `XRANGE (id` and `XTRIM MINID`, both added in 6.2.
const MIN_REDIS_VERSION: (u32, u32) = (6, 2);

/// Check redis version and configuration requirements.
pub async fn check_redis(pool: &RedisConnectionPool) -> Result<(), RedisConnectionError> {
    let mut client = pool.get().await?;

    let info: String = redis::cmd("INFO")
        .arg("server")
        .query_async(&mut *client)
        .await
        .map_err(to_pool_error)?;
    let version = parse_redis_version(&info).ok_or_else(|| {
        to_pool_error(RedisRawError::from((
            ErrorKind::InvalidClientConfig,
            "could not find redis_version in INFO server",
        )))
    })?;
    if version < MIN_REDIS_VERSION {
        return Err(to_pool_error(RedisRawError::from((
            ErrorKind::InvalidClientConfig,
            "Redis is older than the minimum supported version",
            format!(
                "found {}.{}, need at least {}.{}",
                version.0, version.1, MIN_REDIS_VERSION.0, MIN_REDIS_VERSION.1
            ),
        ))));
    }

    match redis::cmd("CONFIG")
        .arg("GET")
        .arg("maxmemory-policy")
        .query_async::<Vec<String>>(&mut *client)
        .await
    {
        Ok(kv) => {
            let policy = kv.get(1).map(String::as_str);
            if policy != Some("noeviction") {
                tracing::warn!(
                    "Redis maxmemory-policy is {policy:?}, not noeviction: under memory pressure, \
                     Redis may silently evict registry rows and unpersisted commands"
                );
            }
        }
        // A restricted user may not be allowed to read server config; that is not fatal here.
        Err(err) => tracing::warn!("Could not check Redis maxmemory-policy: {err:#?}"),
    }

    Ok(())
}

fn to_pool_error(err: RedisRawError) -> RedisConnectionError {
    RunError::User(err)
}

/// Parses `major.minor` from `redis_version:X.Y.Z` in the output of `INFO server`.
fn parse_redis_version(info: &str) -> Option<(u32, u32)> {
    let version = info.lines().find_map(|line| line.strip_prefix("redis_version:"))?;
    let mut parts = version.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn redis_older_than_6_2_fails_the_start() {
        assert!(parse_redis_version("redis_version:6.0.16").unwrap() < MIN_REDIS_VERSION);
        assert!(parse_redis_version("redis_version:5.0.9").unwrap() < MIN_REDIS_VERSION);
    }

    #[test]
    fn redis_at_or_above_6_2_is_accepted() {
        assert!(parse_redis_version("redis_version:6.2.0").unwrap() >= MIN_REDIS_VERSION);
        assert!(parse_redis_version("redis_version:7.4.1").unwrap() >= MIN_REDIS_VERSION);
    }

    #[test]
    fn a_missing_or_malformed_version_line_is_rejected() {
        assert!(parse_redis_version("").is_none());
        assert!(parse_redis_version("redis_mode:standalone\nos:Linux").is_none());
        assert!(parse_redis_version("redis_version:not-a-version").is_none());
    }

    #[test]
    fn the_version_line_is_found_among_others() {
        let info = "# Server\r\nredis_version:6.2.6\r\nos:Linux\r\n";
        assert_eq!(parse_redis_version(info), Some((6, 2)));
    }
}
