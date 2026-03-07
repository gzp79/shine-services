# P0 Tests Implementation Design

**Date**: 2026-03-05
**Status**: Approved
**Scope**: 45 critical tests (Concurrency, Infrastructure Failures, Cookie Security)

## Overview

This design covers implementation of 45 P0 (critical priority) tests to achieve comprehensive production-ready test coverage for the Identity Service. We exclude rate limiting tests (handled by Cloudflare) and focus on:

- **Concurrency & Race Conditions**: 20 tests
- **Database & Infrastructure Failures**: 15 tests
- **Cookie Security**: 10 tests

## Architecture

### Three-Phase Implementation

The implementation follows a risk-progressive approach where each phase builds confidence before adding complexity:

**Phase 1 (Week 1)**: Concurrency tests use existing Playwright infrastructure. Tests fire concurrent HTTP requests via `Promise.all()` to detect race conditions in email confirmations, token rotations, role management, and external linking. No infrastructure changes needed—the identity service already handles concurrent requests, we're just testing that it does so correctly.

**Phase 2 (Week 2)**: Cookie security tests inspect response headers and cookie attributes using Playwright's existing response capture. Tests verify HttpOnly, Secure, SameSite, Domain, Path attributes, plus signature validation and tampering detection. Again, no infrastructure changes—we're validating what the service already sends.

**Phase 3 (Week 3)**: Infrastructure failure tests introduce Toxiproxy as a network proxy between the identity service and PostgreSQL/Redis. Toxiproxy provides an HTTP API to inject failures (timeouts, connection drops, slow queries) during test execution. This requires docker-compose.yml changes but no Dockerfile changes—the identity service is unaware it's talking through a proxy.

Each phase is independently valuable. We can ship Phase 1 and 2 (30 tests) and defer Phase 3 if needed, giving 8.5/10 coverage score vs the full 9/10 with all three phases.

## Test Structure and Patterns

### Concurrency Test Pattern (Phase 1)

Concurrency tests follow a "race and verify" pattern. We fire multiple operations simultaneously using `Promise.all()`, then verify exactly one succeeds or that both succeed with different outcomes. Example structure:

```typescript
test('Concurrent email confirmations handle race gracefully', async ({ api }) => {
    // Setup: Create user and get confirmation token
    const user = await api.testUsers.createLinked(mockAuth);
    const token = await getConfirmationToken(user);

    // Race: Fire two confirmations simultaneously
    const [result1, result2] = await Promise.all([
        api.user.completeConfirmEmailRequest(user.sid, token),
        api.user.completeConfirmEmailRequest(user.sid, token)
    ]);

    // Verify: One succeeds (200), one fails (400 token-already-used)
    const statuses = [result1.status(), result2.status()].sort();
    expect(statuses).toEqual([200, 400]);
});
```

Key elements:
- **No delays or sleeps** - pure concurrent execution
- **Deterministic assertions** - verify final state, not timing
- **Idempotency checks** - safe operations can succeed twice
- **Conflict detection** - unsafe operations must fail one attempt

### Cookie Security Test Pattern (Phase 2)

Cookie tests inspect response objects for security attributes. No special infrastructure—just read what Playwright already captures:

```typescript
test('Session cookies have HttpOnly flag', async ({ api }) => {
    const response = await api.auth.loginWithGuestRequest(null, null, null);
    const cookies = response.cookies();

    expect(cookies.sid).toHaveProperty('httpOnly', true);
    expect(cookies.tid).toHaveProperty('httpOnly', true);
});
```

Tests check: HttpOnly, Secure (in production), SameSite, Domain matching, Path correctness, Max-Age/Expires alignment with TTL, and signature validation by modifying cookie values and expecting rejection.

### Infrastructure Failure Test Pattern (Phase 3)

Failure tests use Toxiproxy's HTTP API to inject faults during test execution:

```typescript
test('PostgreSQL timeout during login fails gracefully', async ({ api, toxiproxy }) => {
    // Start with healthy system
    const user = await api.testUsers.createGuest(); // works

    // Inject 10-second timeout on PostgreSQL proxy
    await toxiproxy.get('postgres').timeout({ timeout: 10000 });

    // Attempt login - should fail with 503
    const response = await api.auth.loginWithGuestRequest(null, null, null);
    expect(response).toHaveStatus(503);

    // Cleanup - restore healthy proxy
    await toxiproxy.get('postgres').resetToxics();

    // Verify system recovers
    const recovery = await api.auth.loginWithGuestRequest(null, null, null);
    expect(recovery).toHaveStatus(200);
});
```

Toxiproxy provides: `timeout()`, `down()`, `slow()`, `limitData()`, `slicer()` for different failure modes. Tests verify the service returns appropriate errors (503, 500) and recovers when the failure clears.

## Test Organization and File Structure

### Phase 1: Concurrency Tests (20 tests across 5 files)

Following the existing pattern (`tests/api-tests/identity/`), we create:

**`user_email_concurrency.ts`** (6 tests)
- Concurrent email confirmation with same token
- Email change during pending confirmation
- Concurrent email changes to same address (conflict detection)
- Email confirmation + email change race
- Multiple users confirming different emails simultaneously
- Email change + user deletion race

**`sessions_concurrency.ts`** (4 tests)
- Concurrent logins from same token creating separate sessions
- Logout during active API request
- Session refresh during logout
- Simultaneous session creation from multiple IPs

**`tokens_concurrency.ts`** (4 tests)
- Concurrent token rotation producing unique tokens
- Token revocation during login attempt
- Concurrent token revocations (idempotent)
- Token rotation + logout race

**`external_links_concurrency.ts`** (3 tests)
- Concurrent linking of same external account (conflict)
- Link and unlink same account concurrently
- Two users linking different accounts simultaneously (should both succeed)

**`roles_concurrency.ts`** (3 tests)
- Concurrent role additions (deduplication)
- Concurrent add and delete of same role
- Multiple admins modifying roles on same user

Each file uses the existing test infrastructure: `import { expect, test } from '$fixtures/setup'`, follows existing naming conventions, and uses the same `MockSmtp`, `OAuth2MockServer`, `OpenIdMockServer` patterns where needed.

### Phase 2: Cookie Security Tests (10 tests in 2 files)

**`cookie_security.ts`** (7 tests)
- HttpOnly flag on sid/tid/eid
- Secure flag enforcement (check in test vs prod configs)
- SameSite attribute (Lax or Strict)
- Domain attribute matching service domain
- Path attribute correctness
- Max-Age alignment with session/token TTL
- Cookie present only on auth endpoints (not on /info/ready)

**`cookie_tampering.ts`** (3 tests)
- Modified cookie value rejected (signature validation)
- Old rotated cookie after rotation window
- Cookie from different user doesn't grant access

### Phase 3: Infrastructure Failure Tests (15 tests in 3 files)

**`database_failures.ts`** (6 tests)
- PostgreSQL connection failure returns 503
- Database timeout during token creation
- Connection pool exhaustion queues or fails gracefully
- Transaction rollback on constraint violation (no orphaned data)
- Slow query doesn't hang requests indefinitely
- Database recovery after outage

**`redis_failures.ts`** (4 tests)
- Redis connection failure degrades gracefully
- Redis key expiration during session read
- Redis write failure during login rolls back transaction
- Redis recovery after outage

**`email_failures.ts`** (5 tests)
- SMTP timeout returns user-friendly error
- SMTP rejection doesn't prevent user registration
- Partial SMTP failure allows retry
- SMTP connection during high load
- Email queue resilience

All files follow the existing test structure and integrate with the current fixtures. No new test runner or framework needed—pure Playwright extensions.

## Toxiproxy Setup (Phase 3 Only)

### Docker Compose Changes

Add Toxiproxy as a service in `services/docker-compose.yml`:

```yaml
toxiproxy:
  image: ghcr.io/shopify/toxiproxy:2.9.0
  ports:
    - 8474:8474  # API port
    - 5433:5433  # Proxied PostgreSQL
    - 6380:6380  # Proxied Redis
  networks:
    - shine
```

Reconfigure the identity service to connect through Toxiproxy by changing the `links` in the `shine` service:

```yaml
shine:
  # ... existing config ...
  links:
    - toxiproxy:postgres.mockbox.com  # Changed from postgres
    - toxiproxy:redis.mockbox.com     # Changed from redis
```

Toxiproxy acts as a transparent proxy—it forwards all traffic normally until we inject faults via its API. The identity service doesn't know it's talking through a proxy.

### Toxiproxy Configuration Script

Create `tests/tools/setup_toxiproxy.ts` to initialize proxies when tests start:

```typescript
import Toxiproxy from 'toxiproxy-node-client';

export async function setupToxiproxy() {
  const toxiproxy = new Toxiproxy('http://localhost:8474');

  // Create PostgreSQL proxy: listen on 5433, forward to postgres:5432
  await toxiproxy.create({
    name: 'postgres',
    listen: '0.0.0.0:5433',
    upstream: 'postgres:5432'
  });

  // Create Redis proxy: listen on 6380, forward to redis:6379
  await toxiproxy.create({
    name: 'redis',
    listen: '0.0.0.0:6380',
    upstream: 'redis:6379'
  });

  return toxiproxy;
}
```

This script runs once in `beforeAll` for Phase 3 tests.

### Test Fixture Extension

Add toxiproxy client to the existing `ServiceTestFixture`:

```typescript
export type ServiceTestFixture = {
  api: Api;
  toxiproxy?: Toxiproxy; // Optional - only available in Phase 3 tests
};
```

Phase 3 test files import and use:

```typescript
test.beforeAll(async ({ toxiproxy }) => {
  toxiproxy = await setupToxiproxy();
});

test.afterEach(async ({ toxiproxy }) => {
  // Clear all toxics after each test to ensure clean state
  await toxiproxy.get('postgres').resetToxics();
  await toxiproxy.get('redis').resetToxics();
});
```

### Minimal Changes Summary

- **docker-compose.yml**: +12 lines (toxiproxy service + link changes)
- **tests/package.json**: +1 line (`toxiproxy-node-client`)
- **tests/tools/setup_toxiproxy.ts**: +30 lines (new file)
- **tests/fixtures/setup.ts**: +2 lines (optional toxiproxy fixture)
- **Dockerfile**: 0 changes
- **GitHub Actions**: 0 changes (docker-compose already used)

Total: ~45 lines of changes for Phase 3 infrastructure. Phases 1 and 2 need zero infrastructure changes.

## Error Handling and Verification Strategies

### Concurrency Test Verification (Phase 1)

Concurrency tests must verify **final state correctness**, not execution order or timing. Three verification strategies:

**1. Conflict Detection (one wins, one fails)**
Used when operations are mutually exclusive (e.g., two users claiming same email):
```typescript
const statuses = [result1.status(), result2.status()].sort();
expect(statuses).toEqual([200, 409]); // One succeeds, one conflicts
```

**2. Idempotent Operations (both can succeed)**
Used when operations are safe to repeat (e.g., adding same role twice):
```typescript
expect(result1).toHaveStatus(200);
expect(result2).toHaveStatus(200);
const roles = await api.user.getRoles(sid, false, userId);
expect(roles).toContain('Role1'); // Added once despite two requests
```

**3. Resource Creation (different outputs)**
Used when operations create distinct resources (e.g., concurrent logins):
```typescript
const sid1 = result1.cookies().sid.value;
const sid2 = result2.cookies().sid.value;
expect(sid1).not.toEqual(sid2); // Different sessions
expect(await api.session.getSessions(sid1)).toHaveLength(2); // Both exist
```

All tests include cleanup verification—check database state after the race to ensure no orphaned data or inconsistent state.

### Cookie Security Verification (Phase 2)

Cookie tests verify security properties at multiple levels:

**Attribute Validation**: Check flags exist and have correct values
**Signature Validation**: Modify signed cookie content and verify rejection
**Domain Validation**: Ensure cookies aren't sent cross-domain
**TTL Validation**: Compare Max-Age against expected session/token lifetime

For environment-dependent attributes (like `Secure` flag), tests check the configuration:
```typescript
test('Secure flag matches environment', async ({ api }) => {
  const response = await api.auth.loginWithGuestRequest(null, null, null);
  const isProduction = process.env.ENVIRONMENT === 'prod';
  expect(response.cookies().sid.secure).toBe(isProduction);
});
```

### Infrastructure Failure Verification (Phase 3)

Failure tests verify three aspects: **graceful degradation**, **appropriate errors**, and **recovery**:

```typescript
test('PostgreSQL timeout fails gracefully', async ({ api, toxiproxy }) => {
  // 1. Inject failure
  await toxiproxy.get('postgres').timeout({ timeout: 10000 });

  // 2. Verify graceful failure (503, not crash or hang)
  const response = await api.auth.loginWithGuestRequest(null, null, null);
  expect(response).toHaveStatus(503);
  expect(await response.parseProblem()).toEqual(
    expect.objectContaining({
      type: 'service-unavailable',
      detail: expect.stringContaining('database')
    })
  );

  // 3. Clear failure
  await toxiproxy.get('postgres').resetToxics();

  // 4. Verify recovery (service resumes normal operation)
  const recovered = await api.auth.loginWithGuestRequest(null, null, null);
  expect(recovered).toHaveStatus(200);
});
```

Critical: Tests must clean up toxics in `afterEach` to prevent failures from leaking between tests. If a test fails mid-execution, the toxic cleanup ensures subsequent tests start clean.

## CI/CD Integration

### Phase 1 & 2: Zero CI Changes

Phases 1 and 2 tests integrate seamlessly into the existing CI pipeline. They're standard Playwright tests that run in the current `pnpm run test:local` command. No workflow changes, no new dependencies (beyond dev dependencies already in package.json).

The existing `.github/workflows/service_ci.yml` workflow already:
- Starts PostgreSQL and Redis via docker-compose
- Builds the identity service Docker image
- Runs Playwright tests
- Publishes test reports

Concurrency and cookie security tests simply add more test files to the `tests/api-tests/identity/` directory. The CI discovers and runs them automatically.

### Phase 3: Minimal CI Changes

For Phase 3 infrastructure failure tests, the CI pipeline needs Toxiproxy. Add Toxiproxy setup in the existing integration test step:

```yaml
- name: Integration test
  run: |
    echo "::group::Starting service in the test environment"
    docker-compose -f ../services/docker-compose.yml -p shine --profile test up -d
    npx tsx tools/setup_toxiproxy.ts  # Add this line
    echo "::endgroup::"
```

Toxiproxy starts as part of docker-compose, setup script configures proxies, then tests run. No changes to the docker build step, no changes to test execution.

### Test Execution Control

Tests can be organized by tags to control which phases run:

```typescript
// Phase 1 tests
test.describe('Concurrency tests', { tag: '@concurrency' }, () => { ... });

// Phase 2 tests
test.describe('Cookie security', { tag: '@security' }, () => { ... });

// Phase 3 tests (require Toxiproxy)
test.describe('Database failures', { tag: '@infrastructure' }, () => { ... });
```

This allows running subsets:
- `pnpm test --grep @concurrency` - Phase 1 only
- `pnpm test --grep-invert @infrastructure` - Skip Phase 3 if Toxiproxy not available
- `pnpm test` - Run all phases (default in CI)

Useful for local development where developers may not have Toxiproxy running.

### CI Workflow Impact

**Phases 1 & 2**: No impact on CI duration—tests run in parallel with existing tests
**Phase 3**: Adds ~2-3 minutes for Toxiproxy startup and failure scenario execution (timeouts, slow queries take time to simulate)

Total CI time: Current + 2-3 minutes for Phase 3. Phases 1 and 2 add negligible time since they're concurrent operations that complete quickly.

## Implementation Order and Timeline

### Phase 1: Concurrency Tests

**Day 1-2**: Email concurrency tests (6 tests in `user_email_concurrency.ts`)
- Start with simplest: concurrent email confirmations with same token
- Build up to complex: email change during confirmation, concurrent conflicts
- These tests use existing MockSmtp infrastructure

**Day 3**: Session and token concurrency (8 tests across 2 files)
- `sessions_concurrency.ts`: Login races, logout during requests
- `tokens_concurrency.ts`: Token rotation races, revocation timing
- Reuse existing session/token API helpers

**Day 4**: External links and roles concurrency (6 tests across 2 files)
- `external_links_concurrency.ts`: OAuth2/OIDC linking conflicts
- `roles_concurrency.ts`: Role modification races
- Requires OAuth2MockServer and admin user setup

**Day 5**: Integration, debugging, and polish
- Run all 20 tests together, fix any race conditions in tests themselves
- Add documentation comments
- Verify tests fail when they should (negative testing)

**Deliverable**: 20 passing concurrency tests, 0 infrastructure changes

### Phase 2: Cookie Security Tests

**Day 1-2**: Cookie attribute tests (7 tests in `cookie_security.ts`)
- HttpOnly, Secure, SameSite, Domain, Path validation
- Max-Age and TTL alignment checks
- Cookie presence/absence verification

**Day 2-3**: Cookie tampering tests (3 tests in `cookie_tampering.ts`)
- Signature validation by modifying cookie content
- Replay attack detection
- Cross-user cookie rejection

**Day 4-5**: Edge cases and integration
- Test cookie behavior across different auth flows (guest, email, OAuth2, OIDC)
- Verify cookies cleared on logout
- Document expected cookie security properties

**Deliverable**: 10 passing cookie security tests, 0 infrastructure changes

### Phase 3: Infrastructure Failure Tests

**Day 1**: Toxiproxy setup
- Add Toxiproxy to docker-compose.yml
- Create setup script
- Verify proxies work without toxics (transparent forwarding)
- Update CI workflow

**Day 2-3**: Database failure tests (6 tests in `database_failures.ts`)
- Start simple: connection failure (complete down)
- Add complexity: timeouts, slow queries
- Most complex: connection pool exhaustion

**Day 4**: Redis and email failure tests (9 tests across 2 files)
- `redis_failures.ts`: Redis down, key expiration, write failures
- `email_failures.ts`: SMTP timeouts, rejections, partial failures

**Day 5**: Recovery testing and cleanup
- Verify all tests clean up toxics properly
- Test that service recovers after failures clear
- Stress test: multiple failures in sequence

**Deliverable**: 15 passing infrastructure failure tests, Toxiproxy integrated

## Risk Mitigation

**Risk 1: Tests are flaky due to timing**
- Mitigation: Use deterministic assertions (final state), not timing-based checks
- Retry strategy: CI already has `retries: 2` for flaky tests
- Validation: Run each test 10 times locally before merging

**Risk 2: Toxiproxy setup complexity blocks Phase 3**
- Mitigation: Phases 1 & 2 deliver value independently (30 tests)
- Fallback: Can skip Phase 3 if blocked, still achieve 8.5/10 coverage
- Early validation: Set up Toxiproxy on day 1 of Phase 3, fail fast if issues

**Risk 3: Concurrency tests expose real bugs in the service**
- Mitigation: This is actually the goal! But could delay test completion
- Strategy: Document bugs found, create issues, decide whether to fix or skip test
- Pragmatic approach: Some race conditions may be acceptable (log and continue)

**Risk 4: CI time increases significantly with Phase 3**
- Mitigation: Tag Phase 3 tests separately, can run less frequently (nightly)
- Optimization: Run failure tests in parallel where possible
- Monitoring: Measure actual CI time impact before full rollout

## Success Criteria

**Phase 1 Complete**: 20 concurrency tests passing in CI, no new infrastructure
**Phase 2 Complete**: 10 cookie security tests passing in CI, no new infrastructure
**Phase 3 Complete**: 15 infrastructure failure tests passing in CI, Toxiproxy operational

**Overall Success**: 45/45 P0 tests passing, test coverage score 9/10, CI stable

## Next Steps

1. Approve this design document
2. Transition to implementation planning (writing-plans skill)
3. Begin Phase 1 implementation
