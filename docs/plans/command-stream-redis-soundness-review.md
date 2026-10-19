# Redis Command Stream Soundness Review

**Date:** 2026-10-19  
**Reviewed:** `services/builder/src/repositories/command_stream/redis/redis_command_stream_db.rs`  
**Effort Level:** xhigh  
**Status:** Critical issues identified

## Executive Summary

Comprehensive analysis of the Redis command stream implementation identified **12 findings** across soundness, Redis best practices, and design optimality. Critical issues include script atomicity violations, missing error handling, and data corruption risks.

## Critical Soundness Issues (Must Fix)

### 1. HYDRATE Script Violates "Validate First, Write Last"
**File:** `scripts.rs:43`  
**Severity:** Critical

The HYDRATE script deletes log and ids before writing new data, violating the stated principle.

**Failure Scenario:**  
If the script runs out of memory or hits a Lua error after `DEL` (line 43) but before completing the write loop (lines 44-49), the log and dedupe sets are wiped but not restored. The stream has meta saying `version=10` but an empty log. Subsequent reads return no commands despite `meta.version > 0`, and the `check_tail` validation on future hydrations fails because the tail cannot end at the claimed head.

**Impact:** Data loss, stream becomes unreadable

### 2. Missing Nil Checks in MARK_PERSISTED
**File:** `scripts.rs:73`  
**Severity:** Critical

Script doesn't handle missing `persisted` field, causing Lua error instead of returning clean error code.

**Failure Scenario:**  
If the meta hash exists but `persisted` field is missing (data corruption or old schema), `tonumber(meta[3])` returns nil at line 74, then `v <= nil` throws Lua error 'attempt to compare number with nil'. The Rust side gets a generic `RedisError` instead of `MarkOutcome::Refused`, making debugging harder and potentially crashing retry logic that expects clean enum variants.

**Impact:** Unhandled exceptions, difficult debugging

### 3. Missing Nil Checks in RETIRE_ACTIVE
**File:** `scripts.rs:134`  
**Severity:** Critical

Script doesn't handle missing `persisted` field on comparison.

**Failure Scenario:**  
If meta exists but `persisted` field is missing (corruption), `meta[2]` is nil, `tonumber(meta[2])` returns nil, and `tonumber(meta[1]) > nil` throws Lua error. The orphan sweep (caller of `retire_active`) gets a `RedisError` instead of a boolean, potentially stopping the sweep loop and leaking active-set entries.

**Impact:** Memory leaks, orphan accumulation

### 4. parse_entry Creates Version Gaps
**File:** `redis_command_stream_db.rs:309`  
**Severity:** Critical

Parse failures cause `read_after` to skip entries, creating version gaps in returned vector.

**Failure Scenario:**  
If Redis stream entry at version 5 has corrupt UUID in 'c' field (`parse_str` fails at line 108), `parse_entry` returns None, the error is logged (line 311), and the entry is dropped. `read_after` returns versions [4, 6, 7] with a gap at 5. The worker's cursor logic assumes contiguous versions; a gap breaks the 'entries are [fanned_out+1 .. latest]' invariant, causing the worker to think version 6 is the next version after 4, and to deliver version 6 as version 5 to subscribers, breaking checksums.

**Impact:** State divergence, checksum failures

## Design Flaws

### 5. HYDRATE Unnecessary Meta DEL
**File:** `scripts.rs:50`  
**Severity:** High

Script deletes meta before recreating it with HSET.

**Failure Scenario:**  
If the script fails between `DEL KEYS[1]` (line 50) and `HSET KEYS[1]` (line 51), the meta is gone but log/ids may be partially written. The stream becomes unreadable (`meta()` returns None, `ingest` returns NotReady). HSET would overwrite all three fields anyway, so the DEL adds risk without benefit.

**Recommendation:** Use HSET alone, or DEL after HSET succeeds.

### 6. Generation Field Not Validated
**File:** `redis_command_stream_db.rs:107`  
**Severity:** High

Command generation field is parsed but never validated against stream's current generation.

**Failure Scenario:**  
HYDRATE should wipe all log entries (line 43 in scripts.rs), but if the DEL partially fails (Redis cluster split-brain, or script interrupted), old commands from generation 3 remain in the log. New hydration writes generation 5 to meta but doesn't clear the log fully. `read_after` returns commands tagged with `generation=3` mixed with `generation=5`. The worker replays them, but generation 3 commands may have been replaced by generation 5 (same versions, different command_ids). Replay produces wrong state hash, causing checkpoint failure and stream unavailability.

**Impact:** State hash mismatches, stream unavailability

### 7. Silent Corruption Masking in to_meta
**File:** `redis_command_stream_db.rs:98`  
**Severity:** Medium

`to_meta` silently defaults missing version and persisted fields to 0 without logging corruption warning.

**Failure Scenario:**  
If generation exists but version or persisted don't (partial HSET due to memory pressure or old code), `to_meta` returns `RedisStreamMeta{generation: 5, version: 0, persisted: 0}`. This is invalid (versions start at 1), but no warning is logged. The worker proceeds with version=0, tries to ingest and assigns version 1, potentially diverging from Postgres which knows version=50. The divergence is detected later (snapshot hash mismatch), but the root cause (corrupt meta) is hidden.

**Impact:** Hidden corruption, difficult debugging

### 8. MARK_PERSISTED Lacks Input Validation
**File:** `scripts.rs:86`  
**Severity:** Medium

Script computes trim boundary with `math.min` but doesn't validate `keep_from` is a number.

**Failure Scenario:**  
If the Rust side has a bug and passes `keep_from` as a non-numeric string (e.g., clamping removed), `tonumber(ARGV[4])` returns nil at line 86, `math.min(v+1, nil)` throws Lua error 'bad argument #2 to min (number expected, got nil)'. The `mark_persisted` call fails, the worker's persist cycle stops, and unpersisted commands accumulate until `max_unpersisted` is hit, causing INGEST to return Busy and blocking all new writes to the stream.

**Impact:** Stream becomes unavailable for writes

## Observability Gaps

### 9. Insufficient Error Detail in parse_entry
**File:** `redis_command_stream_db.rs:311`  
**Severity:** Medium

Error log lacks detail about which field failed to parse.

**Failure Scenario:**  
Entry parsing fails for version 42. The log says '[chat-global] Unreadable command stream entry 42-0' but doesn't say whether 'c' (command_id), 'u' (user_id), 'g' (generation), or 'd' (data) caused the failure. Ops debugging a production incident with 10000 corrupt entries must manually XRANGE and inspect each entry to find the pattern (e.g., 'all user_id fields are corrupt'). This costs 30 minutes instead of 30 seconds.

**Impact:** Slow incident response

### 10. Silent Drops in idle_streams
**File:** `redis_command_stream_db.rs:349`  
**Severity:** Medium

`idle_streams` silently drops unparseable stream IDs returned by Redis without logging.

**Failure Scenario:**  
If a bug in another part of the code writes an invalid stream ID 'chat..global' (double dots) to the active sorted set, `idle_streams` calls IDLE_STREAMS script which returns it, but `StreamId::parse` fails (regex rejects double dots). The `filter_map` drops it silently. The orphan sweep never processes this stream, so it leaks in the active set forever, consuming memory and slowing down future ZRANGEBYSCORE calls.

**Impact:** Memory leaks, performance degradation

### 11. listen_to_wakes Treats Parse Failures as Reconnect
**File:** `redis_command_stream_db.rs:385`  
**Severity:** High

`listen_to_wakes` treats `StreamId` parse failures as None (wake all), indistinguishable from reconnect.

**Failure Scenario:**  
INGEST publishes payload 'chat..global' (invalid ID due to a bug). The listener receives it, `StreamId::parse` fails, handler is called with None. None means 'wake all streams after reconnect', so the dispatcher wakes every resident stream. With 1000 streams and a buggy client sending invalid IDs every 100ms, the dispatcher churns through wake-all cycles, loading every stream from Redis/Postgres repeatedly, causing 100x load spike and exhausting connections.

**Impact:** Severe performance degradation, connection exhaustion

### 12. Defensive Clamping Hides Bugs in read_after
**File:** `redis_command_stream_db.rs:301`  
**Severity:** Low

`read_after` clamps negative 'after' values to 0 with `max(0)`, potentially hiding caller bugs.

**Failure Scenario:**  
A buggy worker computes `after = base_version - 1` where `base_version` is 0 (stream just hydrated with no commands), yielding `after = -1`. `read_after` clamps to 0 and returns all commands. The worker expected no commands (since it asked for versions after -1, which don't exist) but gets [1,2,3], processes them, and advances its version to 3. On the next cycle, it asks for versions after 3, gets [4,5], but its state is already at version 3, causing a gap. The root bug (negative after) is hidden by the defensive `max(0)`.

**Impact:** Hidden bugs in calling code

## Redis Best Practices Assessment

### ✅ Strengths
- Scripts are used for atomicity (good)
- Appropriate data structures (Streams, Hashes, Sorted Sets)
- TTL management prevents memory leaks
- Connection pooling
- TTL refresh throttling (once/second) is a good optimization

### ❌ Weaknesses
- Scripts don't follow "validate first, write last" (HYDRATE)
- Error handling could be more defensive (nil checks)
- Scripts assume single-primary Redis (no Redis Cluster support)

## Optimality Assessment

**Overall Architecture:** Sound for single-primary Redis

**Main Limitation:** Requires single-primary Redis instance, not Redis Cluster compatible (scripts assume single instance for atomicity)

**Script Complexity:** Justified by atomicity requirements

## Recommendations

### Immediate Actions (Critical)
1. Fix HYDRATE script to validate first, write last
2. Add nil checks to MARK_PERSISTED and RETIRE_ACTIVE scripts
3. Make parse_entry errors fatal or add explicit gap handling
4. Validate generation field matches current stream generation

### Short-term (High Priority)
5. Remove unnecessary meta DEL from HYDRATE
6. Add corruption warnings in to_meta
7. Add input validation to MARK_PERSISTED for keep_from parameter
8. Improve listen_to_wakes to distinguish parse failures from reconnects

### Medium-term (Observability)
9. Enhance parse_entry error logging with field-specific details
10. Add logging for dropped stream IDs in idle_streams
11. Consider replacing defensive clamping with assertions in read_after
