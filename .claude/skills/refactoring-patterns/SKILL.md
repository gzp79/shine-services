---
name: refactoring-patterns
description: Use when refactoring auth pages or handlers, extracting validation logic, cleaning duplicated code, implementing new auth flows, or encountering ownership/trait bound issues
---

# Identity Service Refactoring Patterns

## When to Use This Skill
- Refactoring auth pages or handlers
- Extracting common validation logic
- Cleaning up duplicated code patterns
- Implementing new auth flows

## Auth Page Handler Pattern

**Location**: `services/identity/src/routes/auth/pages/`

**Standard Structure** (see `email_login.rs` as reference):
```rust
pub async fn handler_name(
    State(state): State<AppState>,
    auth_session: AuthSession,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
    // ... other extractors
) -> AuthPage {
    // 1. Create request helper
    let req = AuthPageRequest::new(&state, auth_session);

    // 2. Validate query
    let query = match req.validate_query(query) {
        Ok(q) => q,
        Err(page) => return page,
    };

    // 3. Validate redirect URLs
    if let Some(page) = req.validate_redirect_urls(query.redirect_url.as_ref(), query.error_url.as_ref()) {
        return page;
    }

    // 4. Validate captcha (if needed)
    if let Some(page) = req.validate_captcha(query.captcha.as_deref(), query.error_url.as_ref()).await {
        return page;
    }

    // 5. Clear auth state (if needed)
    let req = req.clear_auth_state().await;

    // 6. Business logic
    let result = match some_operation().await {
        Ok(r) => r,
        Err(err) => return req.error_page(err, query.error_url.as_ref()),
    };

    // 7. Return response
    PageUtils::new(&state).redirect(req.into_auth_session(), query.redirect_url.as_ref(), None)
}
```

## AuthPageRequest Helper Methods

**Located at**: `services/identity/src/routes/auth/auth_page_request.rs`

**Available Methods**:
- `validate_query<T>()` - Validates and deserializes query params, returns `Result<T, AuthPage>`
- `validate_redirect_urls()` - Checks redirect and error URLs, returns `Option<AuthPage>`
- `validate_captcha()` - Async captcha validation, returns `Option<AuthPage>`
- `clear_auth_state()` - Consumes self, revokes session/access, returns new `AuthPageRequest`
- `error_page()` - Creates error response with optional error URL redirect
- `auth_session()` / `into_auth_session()` - Access/consume session data
- `state()` - Access AppState reference

**Return Pattern**:
- `Option<AuthPage>`: None = success (continue), Some = error (early return)
- Use `if let Some(page) = ...` for early returns
- Validation methods take `&self`, mutation methods consume `self`

## Common Pitfalls

### 1. Ownership/Borrowing Issues
**Problem**: Cannot move `req` after borrowing
```rust
// DON'T DO THIS:
let state_ref = req.state();
let session = req.into_auth_session(); // Error: req is borrowed
PageUtils::new(state_ref).redirect(session, ...)
```

**Solution**: Extract what you need before consuming
```rust
let state_ref = req.state();
let cloned_state = state_ref.clone(); // or use before consuming
let session = req.into_auth_session();
PageUtils::new(&cloned_state).redirect(session, ...)
```

### 2. Generic Trait Bounds
**Problem**: Missing trait bounds on generic validation
```rust
pub fn validate_query<T>(&self, query: Result<ValidatedQuery<T>, ...>) -> Result<T, AuthPage> {
    // Error: T doesn't implement Deserialize + Validate
}
```

**Solution**: Add proper trait bounds
```rust
pub fn validate_query<T>(&self, query: Result<ValidatedQuery<T>, ...>) -> Result<T, AuthPage>
where T: serde::de::DeserializeOwned + validator::Validate
```

### 3. Email Storage for External Logins
**Problem**: Emails from OAuth2/OIDC not stored in identity table
```rust
// services/identity/src/services/user_service.rs
let identity = ctx.create_user(user_id, name, None).await?; // Email lost!
```

**Solution**: Extract and validate email from external provider
```rust
let email = external_user
    .email
    .as_ref()
    .filter(|email| email.validate_email()) // Only store valid emails
    .map(|e| (e.as_str(), false)); // false = not confirmed
let identity = ctx.create_user(user_id, name, email).await?;
```

## Extracting Common Patterns

**When to create a helper:**
1. Logic duplicated across 3+ files
2. Validation follows same pattern (query, redirect, captcha, auth)
3. Error handling is consistent

**How to extract:**
1. Identify the pattern and its variations
2. Create helper struct with necessary context (AppState, AuthSession)
3. Use `&self` for reads, consume `self` for state mutations
4. Return `Option<AuthPage>` for optional validations, `Result<T, AuthPage>` for required ones
5. Early-return pattern: `if let Some(page) = validation() { return page; }`

## Testing After Refactoring

**Expected outcomes:**
- Same number of tests should pass before/after
- Test failures indicate either bugs in refactoring OR pre-existing bugs exposed by changes
- Check if tests were passing before by reviewing `.last-run.json`

**Verification steps:**
1. Rebuild: `cargo build -p shine-identity --release`
2. Run service locally (see local-development skill)
3. Run full test suite: `cd tests && pnpm test:local`
4. Compare pass rate before/after
5. Investigate any new failures - may be bugs in original code!

## Architecture Guidelines

**Service Layer Separation:**
- **Routes**: HTTP extraction, minimal logic, delegate to handlers
- **Handlers**: Orchestrate services, compose operations
- **Services**: Business logic, database access, single responsibility

**Don't put business logic in routes:**
- ❌ `routes/auth/pages/email_login.rs` calling multiple services directly
- ✅ `routes/auth/pages/email_login.rs` calling `login_email_handler().send_login_email()`

**Validation belongs at route level:**
- Query validation, redirect URL checks, captcha - all in route handler
- Business logic errors (user not found, token expired) - in services/handlers
