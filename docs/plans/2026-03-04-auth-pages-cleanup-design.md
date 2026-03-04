# Auth Pages Cleanup Design

**Date:** 2026-03-04
**Status:** Approved
**Goal:** Clean up auth pages to better conform to Routes → Handlers → Services architecture and improve maintainability

## Problem Statement

Current auth page architecture has several inconsistencies:

1. **Mixed concerns in AuthUtils:** Contains both validation helpers and orchestration logic (`complete_external_login/link`)
2. **Inline orchestration:** `token_login.rs` has an inline `authenticate()` function that orchestrates multiple handler methods
3. **Thin wrapper handlers:** `LoginTokenHandler` is just a wrapper around `token_service` with no real orchestration
4. **Handler nesting:** Some code paths have handlers calling other handlers instead of services directly
5. **Inconsistent patterns:** Auth pages use different approaches for similar operations

These issues violate the clean architecture principle: Routes → Handlers → Services (flat, no nesting).

## Architecture Overview

### Current State
```
Routes → AuthPageRequest (validation)
      ↓
      → AuthUtils (validation + orchestration)  ← MIXED CONCERNS
      → AuthHandler (token auth orchestration)
      → LoginTokenHandler (thin wrapper)        ← UNNECESSARY
      → LoginEmailHandler (email orchestration)
      ↓
      Services (business logic)
```

### Target State
```
Routes → AuthPageRequest (all validation)       ← CONSOLIDATED
      ↓
      → AuthHandler (token auth orchestration)
      → ExternalLoginHandler (OAuth2/OIDC)      ← NEW
      → LoginEmailHandler (email orchestration)
      ↓
      Services (business logic, no wrappers)
```

### Key Principles

1. **Flat handler structure:** Handlers call services directly, never other handlers
2. **Clear separation:** Routes extract HTTP, handlers orchestrate services, services implement business logic
3. **Handler granularity:** Each handler orchestrates one authentication type (token/external/email)
4. **No thin wrappers:** Remove handlers that just wrap a single service call
5. **Consolidated validation:** All request validation in `AuthPageRequest`

## Design Details

### 1. Handler Changes

#### ExternalLoginHandler (NEW)

**File:** `services/identity/src/handlers/external_login_handler.rs`

**Responsibility:** Orchestrate OAuth2/OIDC authentication flows

**Methods:**
- `complete_external_login(auth_session, fingerprint, site_info, external_user, redirect_url, error_url, create_token) -> AuthPage`
  - Find existing linked account or create new user
  - Create access token if requested
  - Create user session
  - Return redirect or error page
  - Orchestrates: `link_service`, `user_service`, `token_service`, session creation

- `complete_external_link(auth_session, external_user, redirect_url, error_url) -> AuthPage`
  - Add external provider link to existing authenticated user
  - Return redirect or error page
  - Orchestrates: `link_service`

**Constructor:** `new(&AppState) -> Self`

**Why:** These methods currently in `AuthUtils` perform real orchestration (multiple service calls, token/session creation), which is handler responsibility.

#### AuthHandler (EXTEND)

**File:** `services/identity/src/handlers/auth_handler.rs`

**Add method:**
- `authenticate_user(query, auth_header, auth_session, fingerprint) -> Result<AuthenticationSuccess, AuthenticationFailure>`
  - Determine authentication method (query token, header token, cookie token, session refresh)
  - Call appropriate `authenticate_with_*` method
  - Return authentication result

**Why:** Currently implemented as inline function in `token_login.rs`. This orchestration logic belongs in the handler.

#### LoginTokenHandler (DELETE)

**File:** `services/identity/src/handlers/login_token_handler.rs`

**Rationale:** Thin wrapper around `token_service.create_with_retry()` with no orchestration value.

**Migration:** Replace all `state.login_token_handler().create_user_token()` calls with direct `state.token_service().create_with_retry()` calls.

#### LoginEmailHandler (NO CHANGE)

**File:** `services/identity/src/handlers/login_email_handler.rs`

**Keep as-is:** Already orchestrates multiple services (`user_service`, `token_service`, `mailer_service`) - legitimate handler work.

### 2. AuthPageRequest & AuthUtils Consolidation

#### AuthPageRequest (EXTEND)

**File:** `services/identity/src/routes/auth/auth_page_request.rs`

**Add method:**
```rust
/// Validate a single redirect URL against allowed patterns
/// Returns None on success, Some(AuthPage) for early return on error
pub fn validate_redirect_url(&self, property: &'static str, redirect_url: &Url) -> Option<AuthPage>
```

**Update method:**
```rust
pub fn validate_redirect_urls(&self, redirect_url: Option<&Url>, error_url: Option<&Url>) -> Option<AuthPage>
```
- Call `self.validate_redirect_url()` internally instead of `AuthUtils::validate_redirect_url()`

**Why:** Consolidates all request-level validation in one place. Validation is inherently request-level concern, not business logic.

#### AuthUtils (DELETE)

**File:** `services/identity/src/routes/auth/auth_utils.rs`

**Rationale:**
- `validate_redirect_url` → moved to `AuthPageRequest`
- `complete_external_login` → moved to `ExternalLoginHandler`
- `complete_external_link` → moved to `ExternalLoginHandler`
- Nothing remains, delete entire file

**Module cleanup:**
- Remove from `services/identity/src/routes/auth/mod.rs`
- Update all imports across auth routes

### 3. Route Changes

#### Standard Route Pattern

All auth pages should follow this structure:

```rust
pub async fn auth_page_handler(...) -> AuthPage {
    // 1. Create request helper
    let req = AuthPageRequest::new(&state, auth_session);

    // 2. Validate query
    let query = match req.validate_query(query) {
        Ok(q) => q,
        Err(page) => return page,
    };

    // 3. Validate redirects
    if let Some(page) = req.validate_redirect_urls(redirect_url, error_url) {
        return page;
    }

    // 4. Validate captcha (if needed)
    if let Some(page) = req.validate_captcha(captcha, error_url).await {
        return page;
    }

    // 5. Call handler orchestration
    let result = SomeHandler::new(&state).orchestrate_flow(...).await;

    // 6. Return response
    match result {
        Ok(success) => PageUtils::new(&state).redirect(...),
        Err(err) => req.error_page(err, error_url),
    }
}
```

#### Specific Route Updates

**token_login.rs:**
- Delete inline `authenticate()` function
- Call `AuthHandler::new(&state).authenticate_user(...)`
- Replace `state.login_token_handler()` with direct `state.token_service()`

**oauth2_auth.rs:**
- Replace `AuthUtils::new(&state).complete_external_login(...)`
- With `ExternalLoginHandler::new(&state).complete_external_login(...)`
- Same for `complete_external_link()`

**oidc_auth.rs:**
- Same changes as oauth2_auth.rs

**oauth2_login.rs, oauth2_link.rs, oidc_login.rs, oidc_link.rs:**
- Remove AuthUtils imports (if present)
- Already mostly clean

**email_login.rs:**
- No changes (already uses LoginEmailHandler correctly)

**validate.rs, logout.rs, delete_user.rs, guest_login.rs:**
- Replace `state.login_token_handler()` with `state.token_service()` if present
- Update AuthUtils imports if present

### 4. AppState Changes

**Remove factory method:**
```rust
// DELETE this method from AppState
pub fn login_token_handler(&self) -> LoginTokenHandler<...> { ... }
```

**Add factory method:**
```rust
// ADD this method to AppState
pub fn external_login_handler(&self) -> ExternalLoginHandler<...> { ... }
```

## Testing & Verification

### Test Strategy

1. **Compilation check:** All files compile without errors
2. **Integration tests:** Run `cd tests && pnpm test:local` - all tests must pass
3. **Architecture audit:**
   - Verify handlers only call services (grep for handler-to-handler calls)
   - Verify no `AuthUtils` references remain
   - Verify no `LoginTokenHandler` references remain

### Risk Assessment

**Low risk (pure refactoring):**
- Moving methods between files
- Deleting thin wrappers
- Consolidating validation

**Requires careful testing:**
- Token authentication flows (query/header/cookie/session)
- OAuth2/OIDC login and registration
- OAuth2/OIDC account linking

### Rollback Plan

- Work on existing `refactor` branch
- All logic is moved, not rewritten (behavior identical)
- If tests fail, investigate before continuing
- Can revert branch if fundamental issues discovered

## Implementation Order

1. Create `ExternalLoginHandler` with methods from `AuthUtils`
2. Add `authenticate_user()` to `AuthHandler`
3. Extend `AuthPageRequest` with `validate_redirect_url()`
4. Update all auth routes to use new handlers
5. Delete `AuthUtils`
6. Delete `LoginTokenHandler`
7. Update AppState factory methods
8. Run tests and verify

## Success Criteria

- [ ] All auth pages follow consistent pattern
- [ ] Handlers only call services (flat structure)
- [ ] All validation in AuthPageRequest
- [ ] No AuthUtils or LoginTokenHandler references
- [ ] All existing tests pass
- [ ] Code is more maintainable and easier to understand

## Files to Modify

**New files:**
- `services/identity/src/handlers/external_login_handler.rs`

**Modified files:**
- `services/identity/src/handlers/auth_handler.rs`
- `services/identity/src/handlers/mod.rs`
- `services/identity/src/routes/auth/auth_page_request.rs`
- `services/identity/src/routes/auth/mod.rs`
- `services/identity/src/routes/auth/pages/token_login.rs`
- `services/identity/src/routes/auth/pages/oauth2_auth.rs`
- `services/identity/src/routes/auth/pages/oauth2_login.rs`
- `services/identity/src/routes/auth/pages/oauth2_link.rs`
- `services/identity/src/routes/auth/pages/oidc_auth.rs`
- `services/identity/src/routes/auth/pages/oidc_login.rs`
- `services/identity/src/routes/auth/pages/oidc_link.rs`
- `services/identity/src/routes/auth/pages/validate.rs`
- `services/identity/src/routes/auth/pages/logout.rs`
- `services/identity/src/routes/auth/pages/delete_user.rs`
- `services/identity/src/routes/auth/pages/guest_login.rs`
- `services/identity/src/app_state.rs`

**Deleted files:**
- `services/identity/src/handlers/login_token_handler.rs`
- `services/identity/src/routes/auth/auth_utils.rs`
