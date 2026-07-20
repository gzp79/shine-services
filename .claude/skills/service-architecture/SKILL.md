---
name: service-architecture
description: >
  Architecture guide for Rust services (identity, builder) in shine-services.
  Use when adding or modifying routes, handlers, services, or repositories —
  or reviewing any code under services/. Covers layer responsibilities,
  handler struct patterns, AppState wiring, and trait/concrete-type rules.
---

# Service Architecture

Clean architecture: Routes → Handlers → Services → Repositories/Integration, with `settings` as a shared normalized configuration module.

## Layer responsibilities

| Layer | Responsibility |
|---|---|
| **Routes** | HTTP extraction, validation (`AuthPageRequest`), response formatting |
| **Handlers** | Borrow services/handlers and orchestrate app workflows |
| **Services** | Owning, flat application logic units (User, Token, Link, Role, Session, Mail) |
| **Repositories** | **DB-oriented persistence only** (`PgIdentityDb`, `RedisSessionDb`) |
| **Integration** | Third-party adapters/clients (`mailer`, captcha, provider clients) |
| **Settings** | Normalized/validated runtime configuration (`IdentitySettings`) |

## Config naming convention

- `*Config`: raw loaded config (serde/env/file input shape)
- `*Settings`: selected/validated/compiled runtime values (for example compiled `Regex`, parsed `Url`, derived lists)

## Handler struct: borrow services, generic over DB types

Handlers are generic over the database type(s) they need. Each service field is a
shared borrow `&'a Service<IDB>`.

```rust
pub struct AuthHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    token_service: &'a TokenService<IDB>,
    user_service: &'a UserService<IDB>,
}

impl<'a, IDB> AuthHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    pub fn new(token_service: &'a TokenService<IDB>, user_service: &'a UserService<IDB>) -> Self {
        AuthHandler { token_service, user_service }
    }
}
```

When a handler composes another handler, include it as a field of the same lifetime:

```rust
pub struct ExternalLoginHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    page_handler: AuthPageHandler<'a>,
    user_session_handler: UserSessionHandler<'a, IDB, SDB>,
    token_service: &'a TokenService<IDB>,
    // ...
}
```

**Never store `&'a AppState` as a field.** Replace with the specific service borrows
the handler actually uses.

## AppState factory methods: always concrete types

Each handler gets a factory method on `AppState` colocated in the handler's own file
(in an `impl AppState` block at the bottom). Return types must name concrete DB types —
never `impl IdentityDb` or `impl SessionDb`.

```rust
// CORRECT
impl AppState {
    pub fn auth_handler(&self) -> AuthHandler<'_, PgIdentityDb> {
        AuthHandler::new(self.token_service(), self.user_service())
    }

    pub fn user_session_handler(&self) -> UserSessionHandler<'_, PgIdentityDb, RedisSessionDb> {
        UserSessionHandler::new(
            self.user_service(),
            self.link_service(),
            self.role_service(),
            self.session_service(),
        )
    }
}

// WRONG — impl Trait in return position creates an opaque type that won't
// unify with concrete types when the handler is composed with other services
impl AppState {
    pub fn auth_handler(&self) -> AuthHandler<'_, impl IdentityDb> { ... }
}
```

**Why:** When a handler is composed inside another handler that also takes
`&TokenService<PgIdentityDb>`, the compiler must unify `IDB`. An opaque `impl IdentityDb`
from one factory won't unify with `PgIdentityDb`, causing type errors. Concrete types
make this work.

## AppState accessors: concrete and explicit

```rust
pub fn user_service(&self) -> &UserService<PgIdentityDb> { &self.0.user_service }
pub fn token_service(&self) -> &TokenService<PgIdentityDb> { &self.0.token_service }
pub fn session_service(&self) -> &SessionService<RedisSessionDb> { &self.0.session_service }
pub fn settings(&self) -> &IdentitySettings { &self.0.settings }
pub fn captcha_validator(&self) -> &CaptchaValidator { &self.0.captcha_validator }
```

`impl Trait` is acceptable only for accessors that construct a value on the fly (not a
reference to a stored field), e.g. `MailerService::new(...)`.

## File locations

| Thing | Location |
|---|---|
| Handler struct + `impl AppState` factory | `services/identity/src/handlers/<name>_handler.rs` |
| Handler export | `handlers/mod.rs` |
| Service | `services/identity/src/services/<name>_service.rs` |
| Settings | `services/identity/src/settings/mod.rs` |
| Integration adapters | `services/identity/src/integration/**` |
| Repository trait (DB only) | `services/identity/src/repositories/<name>_db.rs` |
| Route | `services/identity/src/routes/<name>.rs` |

## Call sites

Routes construct handlers via the factory:

```rust
// CORRECT
let result = state.auth_handler()
    .authenticate_user(&state, ...)
    .await;

// WRONG
let result = AuthHandler::new(&state)
    .authenticate_user(&state, ...)
    .await;
```

## Type-level flow control

| Type | Meaning |
|---|---|
| `Option<AuthPage>` | Validation gate — `None` = continue, `Some(page)` = show error and stop |
| `Result<T, AuthPage>` | Extraction — `Ok(value)` = continue, `Err(page)` = show error and stop |
| Consuming `self` | State transition — prevents reuse after state change |

Usage: `if let Some(err) = req.validate_query(query) { return Ok(err) }`

## Summary of rules

| Rule | Reason |
|---|---|
| Handlers borrow services, not `AppState` | Decouples handler from full state; makes dependencies explicit |
| Handlers may compose other handlers | Keeps higher-level workflows reusable while preserving borrowed dependencies |
| Services are owning and flat | Keeps reusable app logic cohesive; avoids hidden orchestration chains |
| Repositories are DB-only | Prevents persistence abstractions from becoming generic "infra" buckets |
| Integrations hold third-party adapters | Keeps external API concerns separate from DB repositories |
| Settings module stores normalized runtime config | Separates raw config loading from runtime-safe values |
| Generic over `IDB`/`SDB` trait bounds | Keeps handlers testable and DB-agnostic in their own logic |
| Factory on `AppState` returns concrete types | Prevents opaque-type unification errors when composing handlers |
| Factory colocated in handler file | Easy to find; keeps construction logic next to the struct |
| `impl IdentityDb` banned in `impl AppState` factory returns | See concrete types row above |

## Ref docs

| Doc | Load when |
|---|---|
| `docs/services/identity/email-normalization.html` | working on email validation, normalisation, or login credential logic |
| `docs/shared/db-migrations.html` | adding or reviewing database migrations |
