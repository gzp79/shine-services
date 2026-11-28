# Identity service: security and robustness audit

Non-SDP investigation notes. Not kept in sync with the code.

Fresh audit of `services/identity`: HTTP routes, auth handlers, services, PostgreSQL/Redis repositories, SQL migrations, the identity-specific OAuth2/OIDC/CAPTCHA/SMTP integrations, and directly used local `shine-infra` session-cookie extraction, permission, event-bus, web and DB-pool code. No Git commands were used. No repository documentation was used other than `docs/plans/builder-security-audit.md`, read only to match this report's format. No network, live system, install, build, clippy, or test was run.

**Status:** `NEEDS ATTENTION`. Five confirmed findings: one High, three Medium, and one Low. This is not a statement that the service is secure; see [Limitations](#limitations).

**Line numbers** are pointers into the current code tree and may drift.

## Summary

| ID | Sev | Area | Finding |
|----|-----|------|---------|
| I1 | High | session / deletion | A deleted identity can remain authenticated through a Redis session after failed cleanup or a delete-vs-login race |
| I2 | Medium | authorization / concurrency | Revoked roles can remain effective through stale Redis session snapshots after refresh failure or role-change-vs-login race |
| I3 | Medium | CSRF / account lifecycle | A cross-site top-level GET can permanently delete an account when its username is known or guessable |
| I4 | Medium | resource use / persistence | Expired token rows accumulate without bound and an authenticated endpoint returns the full list |
| I5 | Low | resource use / authentication | Supplying an Authorization header to role APIs triggers a costly bcrypt check before permissions are decided |

---

## Authentication, authorization and session state

### I1. Deleted identities can remain authenticated through cached Redis sessions (High)

**Where:** `services/identity/src/services/user_service.rs:96-101`; `services/identity/src/main.rs:31-42`; `services/identity/src/handlers/delete_user_handler.rs:86-98`; `crates/shine-infra/src/session/current_user.rs:108-122`; `crates/shine-infra/src/session/current_user_service.rs:115-148`.

Account deletion deletes the PostgreSQL identity and then publishes an in-process `UserEvent::Deleted`. The subscriber handles deletion by asking `refresh_user_session` to look the identity up and remove sessions when it is absent. Subscriber errors are caught and only logged. The deletion handler also separately calls `remove_all`, but that error too is only logged and deletion still returns success. Redis sessions are not part of the PostgreSQL cascade.

There is also a concrete ordering race even when Redis is healthy: login reads identity/role information before storing its session (`CredentialHandler::establish` → `UserSessionHandler::create_user_session`). Deletion can commit and its event handler can scan/remove all existing Redis sessions after those reads but before the in-flight login writes its new session. That session is created after the cleanup scan. The response can then establish credentials for the already-deleted identity.

The shared `CheckedCurrentUser` extractor validates the signed cookie against Redis session data and does not query PostgreSQL to confirm that the identity still exists. Successful extraction extends the Redis TTL (`current_user_service.rs:129-148`). Thus a session surviving cleanup or created in the race can continue authenticating and can remain active while requests keep refreshing it.

**Impact:** A deleted account can continue to authenticate with cached permissions, potentially including `SuperAdmin`, in the identity service and any service trusting the shared current-user session cache.

**Guards checked:** Identity-owned PostgreSQL rows are cascaded, and both event-driven and explicit Redis cleanup are attempted. Neither is atomic with session creation or guaranteed to succeed. The in-memory event bus is not durable; the handler logs failures rather than invalidating authentication. Session verification relies on Redis data alone.

**Fix:** Make deletion and session invalidation robust to concurrent session creation, e.g. a durable identity/session generation or tombstone checked during session creation and authentication, plus durable cleanup/retry. Avoid reporting deletion complete while authorization-bearing sessions may remain valid.

### I2. Revoked roles can remain effective in cached session snapshots (Medium)

**Where:** `services/identity/src/services/role_service.rs:41-46`; `services/identity/src/main.rs:31-42`; `services/identity/src/handlers/user_session_handler.rs:81-89,109-129`; `services/identity/src/repositories/session/redis/redis_sessions.rs:209-223`; `crates/shine-infra/src/session/current_user.rs:108-122`.

Role removal updates PostgreSQL and publishes `UserEvent::RoleChange`. The subscriber refreshes the user's sessions by first scanning Redis for session hashes and then updating those sessions. `update_all` returns on the first error; the event subscriber logs and suppresses failures. No durable retry, role-version check, or DB-backed authorization check on subsequent requests was found. All route permission checks use `CheckedCurrentUser.roles`, loaded from the Redis snapshot.

A stale role can also result without a Redis error: an in-flight login may read the old roles in `create_user_session`; the role-removal event can complete its `find_all_session_hashes_by_user` scan before that login writes the session; then the login stores a new session carrying the old roles after the refresh has finished. That snapshot has no automatic reconciliation path.

**Impact:** A removed `SuperAdmin` or other privileged role can continue granting permissions on an existing or racing session. If requests continue, session TTL refresh makes the stale authorization state potentially unbounded.

**Guards checked:** Role data is updated in PostgreSQL before event publication, and the event handler attempts a session refresh. The Redis snapshot scan/update is not atomic with session creation, errors are swallowed, and request authorization does not compare the snapshot with authoritative role state.

**Fix:** Couple role/session versions, invalidate sessions on privilege removal, or use durable refresh/retry and ensure newly created sessions cannot publish a version older than the role change. Authorization should fail closed for stale role snapshots.

### I3. Cross-site GET can permanently delete a signed-in user's identity (Medium)

**Where:** `services/identity/src/routes/auth/pages/delete_user.rs:26-33,40-64`; confirmation and mutation at `services/identity/src/handlers/delete_user_handler.rs:68-98`; cookie policy at `services/identity/src/routes/auth/auth_session.rs:128-139,378-383`.

The account-deletion page is registered as `GET /auth/delete`. The handler deletes the current account once `confirmation` equals the user's session name and the session is present in Redis. The session cookie is issued with `SameSite::Lax`, which permits cookies on cross-site top-level safe-method navigations. A site can therefore induce a logged-in victim to navigate to the deletion URL with a known or guessable username in the query. The route permanently removes the identity and then clears its session keys.

**Impact:** An attacker who knows or guesses the victim's username can trigger irreversible account deletion through a cross-site link, redirect, or other top-level navigation.

**Guards checked:** Username confirmation is not an unguessable CSRF token; session validation authenticates the victim but does not prove intent. No method restriction, CSRF token, Origin/Referer validation, or other same-origin check exists on this route. Redirect allowlisting only applies to the post-action destination. `HttpOnly`/`Secure` do not prevent navigation-based CSRF.

**Fix / follow-up:** Add a session-bound CSRF-token GET endpoint that the Svelte client can call from the public browser using the existing session cookie, then change account deletion to `POST` and validate the token plus the username confirmation. Keep the existing exact-origin credentialed CORS allowlist and consider validating `Origin` on deletion as defense in depth. The token may be visible to the legitimate Svelte client; do not expose the HMAC secret or place the token in a URL. Preserve the current client redirect by returning the same HTML after the POST: the browser can process `Set-Cookie` and follow the existing meta refresh to the validated destination with a GET.

---

## Other state-changing page GETs

A route-by-route method review found no other GET page that performs an irreversible identity deletion. There are, however, other GETs with side effects worth tracking separately from I3:

| Route | Reachable side effect | Assessment |
|---|---|---|
| `GET /auth/logout` (`services/identity/src/routes/auth/pages/logout.rs:26-61`; `services/identity/src/handlers/logout_handler.rs:45-92`) | Removes the current Redis session and access token. With `terminateAll=true`, removes all sessions and non-API tokens. | Similar unsafe-method/logout-CSRF concern. Recommend requiring POST; protect `terminateAll` with CSRF validation and explicit intent. Its impact is forced logout / availability rather than account deletion. PostgreSQL token revocation followed by Redis session revocation is an acceptable cross-store order; if Redis cleanup fails, preserve the current session/cookies so the user can retry. |
| `GET /auth/guest/login` (`services/identity/src/routes/auth/pages/guest_login.rs:25-78`) | Creates a guest identity, access token, and session after CAPTCHA validation. | Side-effecting login/registration navigation. Keep the CAPTCHA gate and consider POST if the client flow permits; this audit did not confirm a bypass of CAPTCHA. |
| `GET /auth/email/login` (`services/identity/src/routes/auth/pages/email_login.rs:32-94`) | Can create an identity, create a login token, and send email after CAPTCHA validation. | Side-effecting login initiation. POST is semantically cleaner and avoids surprise state changes from navigation; no CAPTCHA bypass was confirmed in this pass. |
| `GET /auth/{provider}/login` for OAuth2/OIDC (`oauth2_login.rs`, `oidc_login.rs`) | After CAPTCHA, may revoke existing browser session/access token and write an external-login cookie before redirecting to the provider. | Browser-navigation flow by design. Assess session-revocation/forced-login behavior with the client; do not blindly change provider callbacks, which are GET return routes protected by state/PKCE (and OIDC nonce). |
| `GET /auth/{provider}/link` and provider `/auth` callbacks | Link initiation writes signed transient flow state; callback completes the link after state/PKCE validation. | Linking is a persistent mutation only at the callback, whose state is bound to the signed external-login cookie. No additional confirmed CSRF defect found in this targeted pass. |

**Remember for the follow-up:**
- [ ] Add authenticated CSRF-token issuance for the browser client (session-bound, delete-purpose-specific, no-store response).
- [ ] Change account deletion from GET to POST; require and verify the token and keep the existing username confirmation.
- [ ] Preserve the post-delete HTML/meta-refresh response and validated redirect destination.
- [ ] Convert logout to POST; require CSRF validation and explicit intent for `terminateAll`.
- [ ] For `terminateAll`, revoke PostgreSQL long-lived/non-API tokens first, then Redis sessions. If Redis cleanup fails, do not revoke the current session or clear its cookies; return failure so the still-authenticated user can retry. Cross-store atomicity is not required for this sequence.
- [ ] Decide whether guest/email login initiation should use POST; keep CAPTCHA validation in either method.
- [ ] Retain GET for OAuth/OIDC provider callbacks unless the protocol/client integration is deliberately redesigned.

## Persistence and resource use

### I4. Expired token rows accumulate without bound and listing is unpaginated (Medium)

**Where:** `services/identity/src/repositories/identity/pg/pg_tokens.rs:109-117,134-139`; token creation `services/identity/src/routes/auth/api/tokens.rs:116-158`; token listing `services/identity/src/routes/auth/api/tokens.rs:253-266`.

An authenticated user can repeatedly create `Persistent` or `SingleAccess` tokens. Each creation inserts a new PostgreSQL row with an expiry, but `ListByUser` fetches every row for the user and neither filters expired rows nor imposes a limit. The scoped repository and route paths contain no expiry purge; token deletion is otherwise explicit, single-use consumption, replacement of unique email tokens, or identity cascade. Listing materializes and serializes the complete set.

**Impact:** Repeated token creation can cause ongoing PostgreSQL row/index growth and make token listing consume increasingly large database, memory and response resources. A single user's list operation can eventually become expensive even after all their tokens have expired.

**Guards checked:** Per-token TTL has configured upper limits, but there is no token-count cap, expiry cleanup, pagination or bounded result. Token listing has no limit in SQL or service code.

**Fix:** Purge expired rows, enforce a per-user active-token cap, and paginate/bound listing.

### I5. Role API header verification permits authenticated CPU amplification (Low)

**Where:** `services/identity/src/routes/identity/api/user_roles.rs:70-93,121-143,182-205`.

The add/get/delete role routes accept an optional Bearer header and, when the master-key hash is configured, call `bcrypt::verify` before returning either success or a missing-permission response. Any authenticated user can send a syntactically valid but incorrect Bearer token and force that expensive password-hash verification on each request, even though their cookie session lacks the role permission. The route code has no rate limit or bounded concurrent verification. Common middleware has CORS/security headers and in-flight telemetry, not a per-user request throttle.

**Impact:** A low-privilege authenticated client can consume disproportionate CPU on the identity service by repeatedly invoking these routes with an Authorization header. Severity is Low because it requires an authenticated account and the actual impact depends on deployment-level request controls that were outside this audit's scope.

**Guards checked:** A valid current-user session is required; bcrypt errors fail closed; incorrect credentials do not grant role permissions. These checks do not avoid the hash computation or bound its frequency.

**Fix:** Apply request throttling/concurrency limits before bcrypt verification, or use a dedicated API-key authentication path that rate-limits verification attempts.

---

## Reviewed as sound / no additional confirmed findings

- **OAuth2/OIDC callback integrity:** authorization state is kept in a signed cookie and compared at callback; PKCE verifier is bound to that cookie; OIDC additionally checks nonce and uses the library's ID-token verifier. Provider HTTP clients disable redirects; certificate validation is on by default, with explicit config to disable it. Discovery is provider-configured, not user-input-derived.
- **Redirect validation:** supplied success/error URLs are parsed as `Url`, checked against compiled anchored regexes, and the same validated URL is carried through the external callback in the signed cookie. The final redirect page embeds the target URL as a Tera `safe` value; trust therefore rests on the allowlist check and anchored configuration pattern, both present in the flow.
- **Token verification:** raw tokens are UUIDv4 values, hashed with SHA-256 before persistence; one-time tokens are atomically consumed via PostgreSQL `DELETE ... RETURNING`; allowed token kinds, expiration and bound fingerprint/email checks are applied in authentication handlers. Token-cookie identity is checked against the DB identity before accepting it.
- **Authorization and object scoping:** current-user routes use `CheckedCurrentUser`; role APIs bind reads/mutations to the path user id and require a relevant permission or a verified master key; token delete is scoped to the current user; session list reads only the current user's sessions; external-link deletion is scoped to the current user. Guest purge requires `PURGE_GUEST_USERS` and caps each batch at 1000.
- **SQL and input handling:** SQL uses static statements/parameters or the local `QueryBuilder` with bound params; user role names are validated for length/whitespace; search results are capped at 100; public-info batches are validated to 1–100 ids; JSON/query/path extractors validate inputs.
- **Cookie and session key handling:** cookies are signed, Secure and HttpOnly; session keys are random 128-bit values, serialized only in the signed cookie and stored in Redis under a SHA-256 hash. The shared extractor checks the cookie fingerprint against the request fingerprint.
- **Email-change token binding:** the email token is single-use, bound to the requested address, checked for ownership and expiry, and update enforces the unique email constraint. The email confirmation endpoint is authenticated.
- **Memory safety:** no `unsafe`, FFI, or suspicious raw memory operations were found in the reviewed identity paths.

These checks are bounded observations of the traced code paths, not a guarantee against defects outside the reviewed boundaries.

---

## Coverage and checks

**Checked:** Identity route entry points; auth/session/token flows; OAuth2 and OIDC; email login/confirmation/change; account deletion/logout; role and guest-purge authorization; SQL and Redis session persistence; shared session-cookie extraction and permissions; event delivery, update ordering and cancellation/error paths; request bounds and dependency call configuration; relevant panic/unsafe search.

**N/A:** Unsafe/FFI was not found in the reviewed identity/auth/session paths.

**Not reviewed:** Deployment-specific configuration values, proxy/gateway rate limits and network ACLs; external provider behavior; all third-party crate internals (except targeted local dependency API/source checks); unrelated workspace crates; live service behavior; supply-chain advisory databases.

**Checks:** Read-only source, manifest, migration, selected local dependency source, and local infrastructure inspection. No Git commands were used. No network, builds, clippy, tests, installs, scanners, or live systems were used. Builds/tests can execute build scripts and proc macros and were not necessary for static path verification.

## Limitations

- Findings involving Redis cleanup/refresh describe code-level failure and race behavior; deployment-level retry, request limits, or external invalidation not represented in the reviewed source may change exposure.
- The CSRF deletion scenario requires an active victim session and a confirmation value the attacker knows or can guess.
- The token-growth issue requires continued authenticated token creation; no service-side quota or expiry janitor was found in the scoped code.
- The bcrypt amplification issue's operational impact depends on request throttling outside the identity source reviewed here; no such source-level throttle was found.
- This was a manual static audit, not a scanner or runtime verification. A lack of findings in other areas is not evidence those areas are defect-free.
