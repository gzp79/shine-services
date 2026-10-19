use redis::{Arg, Cmd};

/// Commands that (may) park on the server until data arrives. Not supported: they would stall every other
/// command on the shared multiplexed connection, and their duration would be recorded as db latency.
const BLOCKING_COMMANDS: &[&str] = &[
    "BLPOP",
    "BRPOP",
    "BRPOPLPUSH",
    "BLMOVE",
    "BLMPOP",
    "BZPOPMIN",
    "BZPOPMAX",
    "BZMPOP",
    "XREAD",
    "XREADGROUP",
];

/// Commands reported by name in spans and metrics; anything else is reported as `OTHER` to keep the
/// metric label cardinality bounded.
const KNOWN_COMMANDS: &[&str] = &[
    "GET",
    "SET",
    "DEL",
    "EXISTS",
    "EXPIRE",
    "PEXPIRE",
    "TTL",
    "PTTL",
    "KEYS",
    "SCAN",
    "MGET",
    "MSET",
    "GETDEL",
    "GETEX",
    "INCR",
    "INCRBY",
    "DECR",
    "DECRBY",
    "HGET",
    "HSET",
    "HDEL",
    "HGETALL",
    "HMGET",
    "HINCRBY",
    "SADD",
    "SREM",
    "SMEMBERS",
    "SISMEMBER",
    "ZADD",
    "ZREM",
    "ZRANGE",
    "ZRANGEBYSCORE",
    "ZSCORE",
    "LPUSH",
    "RPUSH",
    "LRANGE",
    "LTRIM",
    "XADD",
    "XRANGE",
    "XREVRANGE",
    "XTRIM",
    "XLEN",
    "PUBLISH",
    "WATCH",
    "UNWATCH",
    "MULTI",
    "EXEC",
    "EVAL",
    "EVALSHA",
    "SCRIPT",
    "PING",
    "CLIENT",
];

pub fn redis_command_name(cmd: &Cmd) -> &'static str {
    let Some(Arg::Simple(name)) = cmd.args_iter().next() else {
        return "UNKNOWN";
    };
    BLOCKING_COMMANDS
        .iter()
        .chain(KNOWN_COMMANDS)
        .find(|known| known.as_bytes().eq_ignore_ascii_case(name))
        .copied()
        .unwrap_or("OTHER")
}

fn is_blocking(name: &str) -> bool {
    BLOCKING_COMMANDS.contains(&name)
}

pub fn assert_not_blocking(name: &str) {
    // TODO: Rejects by name only, so XREAD/XREADGROUP are rejected even without BLOCK. Update this check (inspect the args
    // for BLOCK) once a non-blocking XREAD is used on pooled connections.
    if is_blocking(name) {
        panic!("Blocking redis command {name} is not supported on pooled connections, use a dedicated connection with its own instrumentation");
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn command_names_are_case_insensitive_and_bounded() {
        assert_eq!(redis_command_name(&redis::cmd("XADD")), "XADD");
        assert_eq!(redis_command_name(&redis::cmd("xadd")), "XADD");
        assert_eq!(redis_command_name(&redis::cmd("blpop")), "BLPOP");
        assert_eq!(redis_command_name(&redis::cmd("NOT_A_COMMAND")), "OTHER");
    }

    #[test]
    fn blocking_commands_are_detected() {
        assert!(is_blocking(redis_command_name(&redis::cmd("blpop"))));
        assert!(!is_blocking(redis_command_name(&redis::cmd("get"))));
    }
}
