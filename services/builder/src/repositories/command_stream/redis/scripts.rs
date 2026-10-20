use redis::Script;
use std::sync::LazyLock;

// Lua does not roll back on error, so every script checks first and writes last.

/// KEYS: meta, log, ids, active. ARGV: stream_id, command_id, user_id, data, max_unpersisted, ttl, wake channel.
pub static INGEST: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local meta = redis.call('HMGET', KEYS[1], 'generation', 'version', 'persisted')
if not meta[1] or not meta[2] or not meta[3] then return {'not_ready', 0} end
local dup = redis.call('ZSCORE', KEYS[3], ARGV[2])
if dup then return {'duplicate', tonumber(dup)} end
local version = tonumber(meta[2])
if version - tonumber(meta[3]) >= tonumber(ARGV[5]) then return {'busy', version} end
local v = version + 1
redis.call('XADD', KEYS[2], string.format('%d-0', v), 'g', meta[1], 'c', ARGV[2], 'u', ARGV[3], 'd', ARGV[4])
redis.call('HSET', KEYS[1], 'version', v)
redis.call('ZADD', KEYS[3], v, ARGV[2])
-- The TTLs and the activity score are refreshed at most once a second.
local now = redis.call('TIME')
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
local seen = redis.call('ZSCORE', KEYS[4], ARGV[1])
if not seen or now_ms - tonumber(seen) >= 1000 then
  redis.call('ZADD', KEYS[4], now_ms, ARGV[1])
  redis.call('EXPIRE', KEYS[1], ARGV[6])
  redis.call('EXPIRE', KEYS[2], ARGV[6])
  redis.call('EXPIRE', KEYS[3], ARGV[6])
end
redis.call('PUBLISH', ARGV[7], ARGV[1])
return {'ok', v}
"#,
    )
});

/// KEYS: meta, log, ids, active. ARGV: generation, head, ttl, stream_id, then 5 per entry:
/// version, command_id, user_id, generation, data.
pub static HYDRATE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local current = redis.call('HGET', KEYS[1], 'generation')
if current and tonumber(current) >= tonumber(ARGV[1]) then return 0 end
redis.call('DEL', KEYS[2], KEYS[3])
local i = 5
while i <= #ARGV do
  redis.call('XADD', KEYS[2], ARGV[i] .. '-0', 'g', ARGV[i + 3], 'c', ARGV[i + 1], 'u', ARGV[i + 2], 'd', ARGV[i + 4])
  redis.call('ZADD', KEYS[3], ARGV[i], ARGV[i + 1])
  i = i + 5
end
redis.call('DEL', KEYS[1])
redis.call('HSET', KEYS[1], 'generation', ARGV[1], 'version', ARGV[2], 'persisted', ARGV[2])
redis.call('EXPIRE', KEYS[1], ARGV[3])
redis.call('EXPIRE', KEYS[2], ARGV[3])
redis.call('EXPIRE', KEYS[3], ARGV[3])
-- Nothing is unpersisted now; the next write lists the stream again and refreshes the TTLs.
redis.call('ZREM', KEYS[4], ARGV[4])
return 1
"#,
    )
});

/// KEYS: meta, log, ids. ARGV: generation, version, command_id, keep_from.
/// Returns 1 when marked, 0 when nothing changed, -1 when this copy disagrees with what was persisted:
/// it is behind `version`, or holds another command there (it went back in time and reassigned it).
/// Trims the commands below `keep_from` (the version of the oldest stored snapshot), but never above
/// what is persisted, so no unpersisted command is ever trimmed.
pub static MARK_PERSISTED: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local meta = redis.call('HMGET', KEYS[1], 'generation', 'version', 'persisted')
if not meta[1] or meta[1] ~= ARGV[1] then return 0 end
if not meta[2] or not meta[3] then return -1 end
local version = tonumber(meta[2])
local v = tonumber(ARGV[2])
if v <= tonumber(meta[3]) then return 0 end
if v > version then return -1 end
local id = string.format('%d-0', v)
local entry = redis.call('XRANGE', KEYS[2], id, id)
if #entry == 0 then return -1 end
local fields = entry[1][2]
local command_id = nil
for i = 1, #fields, 2 do
  if fields[i] == 'c' then command_id = fields[i + 1] end
end
if command_id ~= ARGV[3] then return -1 end
redis.call('HSET', KEYS[1], 'persisted', v)
local cut = math.min(v + 1, tonumber(ARGV[4]))
if cut > 1 then
  redis.call('XTRIM', KEYS[2], 'MINID', string.format('%d-0', cut))
  redis.call('ZREMRANGEBYSCORE', KEYS[3], '-inf', string.format('(%d', cut))
end
return 1
"#,
    )
});

/// KEYS: meta, log, ids. ARGV: generation.
pub static RESET: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
if redis.call('HGET', KEYS[1], 'generation') ~= ARGV[1] then return 0 end
redis.call('DEL', KEYS[1], KEYS[2], KEYS[3])
return 1
"#,
    )
});

/// KEYS: lock. ARGV: token.
pub static UNLOCK: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('DEL', KEYS[1]) end
return 0
"#,
    )
});

/// KEYS: active. ARGV: idle_ms, limit. The streams without writes for at least idle_ms, oldest first.
pub static IDLE_STREAMS: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local now = redis.call('TIME')
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
return redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', now_ms - tonumber(ARGV[1]), 'LIMIT', 0, tonumber(ARGV[2]))
"#,
    )
});

/// KEYS: active, meta. ARGV: stream_id. Unlists the stream when nothing is unpersisted (atomic with
/// INGEST, which lists it again on its next write).
pub static RETIRE_ACTIVE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local meta = redis.call('HMGET', KEYS[2], 'generation', 'version', 'persisted')
-- Meta exists (the stream was hydrated) but is missing a field it should have: corrupted, so
-- refuse rather than guess at whether anything is still unpersisted.
if meta[1] and not meta[2] then return 0 end
if meta[2] and (not meta[3] or tonumber(meta[2]) > tonumber(meta[3])) then return 0 end
redis.call('ZREM', KEYS[1], ARGV[1])
return 1
"#,
    )
});

/// KEYS: active. ARGV: stream_id. Moves a listed stream behind the other idle streams.
pub static REQUEUE_ACTIVE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local now = redis.call('TIME')
return redis.call('ZADD', KEYS[1], 'XX', tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000), ARGV[1])
"#,
    )
});
