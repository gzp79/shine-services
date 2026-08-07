---
name: api-test-writing
description: >
  How to write API tests under tests/api-tests/ (per-service and shared) and
  tests/mock-tests/ for shine-services. Use whenever adding or modifying a test file,
  writing a new test scenario, adding a mock, or deciding whether a test needs a real
  logged-in user vs a minted session — including when a brand new service is added to
  the workspace and needs its own test folder wired in. Covers folder layout, available
  mocks, session minting limitations, service isolation rules, resiliency/infrastructure
  test taxonomy, and happy-path/edge-case conventions.
---

# API Test Writing

Tests are Playwright Test used purely as an HTTP/WebSocket test harness (no browser
automation) against the real running Rust services. Today that's identity and builder,
but the suite is built to grow — new services get their own folder and slot into the
shared-test loop, not a one-off. Framework mechanics (fixtures, matchers, running tests)
are covered by the `local-development` skill — this skill is about how to structure and
write the test content itself.

## Folder layout

```
tests/
├── api-tests/
│   ├── shared/      # same test logic run against every service in sharedServices
│   ├── identity/    # identity-only tests (login, sessions, tokens, roles, users, ...)
│   ├── builder/     # builder-only tests
│   └── <new-service>/  # one folder per service, added as services are added
├── mock-tests/      # tests OF the mock tooling itself (see "Test the mocks" below)
├── src/
│   ├── api/         # typed API clients (AuthAPI, UserAPI, TestUserHelper, ...)
│   ├── mocks/       # mock servers (SMTP, OAuth2, OIDC, static file server, SessionMint)
│   └── utils/       # generic helpers — always import via the `$lib/utils` barrel,
│                     # never a submodule directly (e.g. not `$lib/utils/_url`)
└── fixtures/
    ├── setup.ts     # `test`/`expect` + fixtures (api, serviceUrl, adminUser, ...)
    └── expect/      # custom matchers, merged into the exported `expect`
```

**Choosing where a new test file goes:**
- Logic identical across services (health, security headers, CORS) →
  `api-tests/shared/`, looping `for (const serviceName of sharedServices)` and using
  `serviceUrl(serviceName)` to resolve the base URL. This avoids duplicating the same
  assertions once per service.
- Logic specific to one service's routes/behavior → `api-tests/<service-name>/`.
- A test that exercises the mock tooling itself, not a Rust service → `mock-tests/`.

**Adding a new service to the suite:** wiring a new service into the test suite is
adding a value, not writing new plumbing — this is the intended growth path:
1. Add its name to `SharedServiceName` and `sharedServices` in `tests/src/utils/_service.ts`
   so every `shared/` test (health, security headers, CORS, ...) automatically covers it.
2. Add its base URL as a new `ServiceOptions` field in `tests/fixtures/setup.ts` (mirroring
   `identityUrl`/`builderUrl`), and extend the `serviceUrl()` fixture's switch to resolve it.
3. Create `api-tests/<service-name>/` for tests specific to that service's own routes.
4. Only add new mocks/session-minting support if the new service actually needs auth or
   talks to an external dependency the existing mocks don't already cover — most services
   can reuse everything in `src/mocks/` and `src/api/` as-is.

## Available mocks

All external dependencies a service would otherwise call over the network are mocked,
so tests never depend on a real SMTP server, OAuth2/OIDC provider, or asset host being
reachable. Mocks live in `tests/src/mocks/` and are reusable across services — a new
service that logs in users or sends email should reuse these rather than growing its own:

| Mock | Purpose | Typical usage |
|---|---|---|
| `MockSmtp` | Intercepts outgoing email (registration, email confirm, login-link) | `mock.waitMail()` before the triggering request, `await` it after |
| `OAuth2MockServer` | Fakes a generic OAuth2 provider | `api.testUsers.createLinked(mock, {...})` |
| `OpenIDMockServer` / `OpenIdMockServer` | Fakes an OIDC provider | same as above, for OIDC flows |
| `StaticFileServer` | Serves static files, standing in for asset/game hosting | CORS/redirect-target checks |
| `SessionMint` | Writes a session directly into Redis (not an HTTP mock) | see below |

Start/stop mocks around the tests that need them — long-lived ones (SMTP, OIDC) in
`test.beforeAll`/`afterAll` for a shared `describe` block, others per-test in
`beforeEach`/`afterEach` when isolation matters more than setup cost.

**Test the mocks too, at least the happy path.** A mock is part of the test framework —
if it silently breaks (wrong port, bad TLS config, changed response shape), every test
that depends on it fails in confusing ways that look like a service bug. `mock-tests/`
holds tests that exercise a mock directly (e.g. `session_mint.test.ts` round-trips
`SessionMint.addUser`/`updateUser` through real Redis, `static_file_server.test.ts`
checks the static server serves a file with the right headers). When you add a new mock
or change an existing one, add or update its happy-path test here — you don't need
exhaustive edge cases for the mock itself, just enough to know the mock is functioning
before you start debugging "why is my service test failing" and it turns out to be the
mock.

## Session minting vs a real user

`SessionMint` (`tests/src/mocks/session_mint.ts`) writes session data straight into
Redis and signs a matching `sid` cookie, using the same HMAC key the identity service
uses. **It never creates a user in Postgres.** `addUser()` only writes the fields the
session data blob actually holds: `name`, `isEmailConfirmed`, `isLinked`, `roles`, plus a
fingerprint. There's no backing `UserInfo`, no email/identity/token record, nothing a
DB-backed lookup could find.

This means:
- Use `SessionMint` when a test only needs *a valid authenticated session* — e.g.
  authorization/gating checks, or WebSocket origin checks (see
  `api-tests/builder/ws_connect.ts`) — where minting via Redis is far cheaper than a
  full login round trip and no DB consistency is required.
- Reach for a real login flow instead — `api.testUsers.createGuest()` or
  `api.testUsers.createLinked(mock, {...})` from `tests/src/api/test_user.ts` — whenever
  the test needs anything a minted session doesn't back: a DB-backed lookup (e.g.
  `search_identity`), a real email on the user, surviving a call that hits Postgres, or
  anything beyond the four fields above. `createGuest`/`createLinked` drive the actual
  HTTP login/registration requests, so the resulting user is fully real.
- Whatever you write with `SessionMint.addUser`, clean up with
  `mint.teardownCreatedSessions()` in `afterEach`/`afterAll` — every existing usage does
  this, and it's how the fixed mock/Redis state stays test-to-test clean.

## Service isolation — no cross-service, no real external calls

A test for one service must never require another service to be running — tests for
service A do not call into service B, and vice versa. `shared/` tests achieve coverage
across every service by parametrizing over `sharedServices`, not by one test calling
into multiple services. Beyond the service(s) under test, the only infrastructure a
test may depend on is what `services/docker-compose.yml` starts — Postgres, Redis, and
`toxiproxy` for fault injection. Anything else external (email provider, OAuth2/OIDC
provider, asset hosting) must go through one of the mocks above, never a real network
call. This is why the suite can run fully locally with just `docker compose ... up -d`
plus the service binaries — no live credentials, no flaky third-party dependency, and
no requirement to have every other service in the workspace running just to test one.

Fault-injection tests that deliberately break Redis/Postgres via `toxiproxy` (e.g.
`identity/redis_failures.ts`) are tagged `{ tag: ['@infrastructure'] }` at the
`describe` level to mark them as a distinct category from ordinary API tests.

## Resiliency tests — a distinct category, add where it's clean

Beyond feature happy-path/edge-case tests, the suite has a separate taxonomy for
**resiliency**: does the service degrade correctly when its infrastructure (Redis,
Postgres) is unhealthy? These live alongside a service's other tests but tagged
`{ tag: ['@infrastructure'] }` (see `identity/redis_failures.ts`, `database_failures.ts`),
and drive `toxiproxy` (`http://localhost:8474`) directly to disable a proxy or inject
latency, then assert the service fails gracefully (typically `503`) and recovers once the
proxy is restored.

Add a resiliency test whenever a new endpoint has a clear, deterministic failure contract
against its infra dependency — e.g. "if Redis is down, this returns 503" or "if Postgres
is unreachable, this returns 503 and recovers once it's back." That's a real API
contract worth locking in.

**Don't** force a resiliency test where the only way to observe the behavior is a timing
window without a clear API signal — e.g. asserting on exact retry counts, backoff
timing, or a race that only shows up under specific latency. The latency test in
`redis_failures.ts` (`'Redis high latency shall not hang requests'`) is close to this
line and stays justifiable only because it asserts a clear API outcome (503, and a
bounded duration window) rather than an internal timing detail — if a resiliency
scenario can't be reduced to "call the API, assert the status/body," it's more trouble
than it's worth and better left uncovered or tested at a lower level (e.g. a Rust unit
test) instead of forcing something flaky into this suite.

## Writing the test itself

Conventions observed throughout `api-tests/`:

- **Test titles describe expected behavior in "shall" phrasing**: `'Login with invalid
  captcha shall be rejected'`, `'WS connect shall reject missing Origin header'`. Keep
  new tests consistent with this style — it reads as a spec, not a log of what was run.
- **`test.describe('<Feature/Scenario>', { tag: [...] }, ...)`** groups related tests;
  interpolate the service name into the title for `shared/` tests:
  `` `Security headers (${serviceName})` ``.
- **Cover the happy path with one thorough test, not many thin ones.** A happy-path test
  typically checks several facets of one flow together (cookies, response body/problem
  shape, follow-on `getUserInfo` for both `'fast'` and `'full'`) rather than splitting
  each assertion into its own test.
- **Cover edge cases as parametrized tables** where the same assertion applies across
  several inputs: `for (const invalidEmail of invalidEmails) { test(...) }`. Interpolate
  the input into the title so a failure immediately shows which case broke.
- **Use `test.step('<label>', ...)`** to break one test into named sequential steps when
  a single scenario has multiple ordered stages (e.g. login with a token, twice).
- **Capture async side effects before triggering them**: `const mailPromise =
  mock.waitMail(); const response = await api.auth...; const mail = await mailPromise;`
  — start waiting first, then act, then await. Doing it the other way risks missing the
  event.
- **Randomize identifiers** (`randomUUID()`, `generateRandomString()`) for emails,
  usernames, and IDs so tests never collide with each other or with leftover state.
- **Explain the "why" in comments for non-obvious edge cases** — e.g. why raw email is
  stored instead of the normalized form — rather than restating what the code does.
- Use the custom matchers from `fixtures/expect/*` (`toHaveStatus`, `toHaveHeader`,
  `toBeValidSID`, `toBeClearCookie`, `toHaveMailTo`, ...) instead of poking at raw
  response internals.

## Happy path and edge cases — what to include

For any new endpoint or flow, aim to cover both:
- **Happy path**: the intended flow succeeds and every observable side effect is
  correct — status code, response body/problem shape, cookies set/cleared, any email
  sent, and a follow-up `getUserInfo` reflecting the change.
- **Edge cases**: invalid input (malformed values, missing fields, wrong captcha),
  security-relevant boundaries (CORS origin variants, header checks), and failure modes
  the handler is specifically supposed to guard against (e.g. rejecting a request kind
  the endpoint doesn't support). Table-drive these where the assertion shape repeats
  across inputs.

Look at `api-tests/identity/login_email.ts` for a file that does both well — plenty of
edge-case tables up front, then thorough parametrized happy-path coverage.
