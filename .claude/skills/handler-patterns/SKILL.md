---
name: handler-patterns
description: >
  Use when creating or modifying a handler struct, adding an AppState factory method,
  composing handlers from services, or reviewing handler code in shine-identity.
---

# Handler Patterns

Handlers orchestrate services. They must not own `AppState` — instead they borrow
the exact services they need.

## Struct: borrow services, parameterize over DB types

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

When a handler composes another handler, include the inner handler as a field of the
same lifetime `'a`:

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

**Never store `&'a AppState` as a field.** If you find it, replace it with the specific
service borrows the handler actually uses.

## AppState factory methods: always use concrete types

Each handler gets a factory method on `AppState` colocated in the handler's own file
(in an `impl AppState` block at the bottom). Factory return types must name concrete
DB types — never `impl IdentityDb` or `impl SessionDb`.

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

**Why concrete types matter:** If a handler is later used inside another handler that
also takes `&TokenService<PgIdentityDb>`, the compiler must unify the type parameter
`IDB`. An opaque `impl IdentityDb` from one factory won't unify with `PgIdentityDb`
from another, causing a type error even though they're the same underlying type at
runtime. Concrete types make this work correctly.

## AppState service accessors: also concrete

For the same reason, service accessors on `AppState` return concrete types:

```rust
pub fn user_service(&self) -> &UserService<PgIdentityDb> { &self.0.user_service }
pub fn token_service(&self) -> &TokenService<PgIdentityDb> { &self.0.token_service }
pub fn session_service(&self) -> &SessionService<RedisSessionDb> { &self.0.session_service }
```

`impl Trait` is acceptable for accessors whose return type is a value type constructed
on the fly (e.g. `MailerService::new(...)`) rather than a reference to a stored field.

## File location

Each handler lives in `services/identity/src/handlers/<name>_handler.rs` and exports
its struct from `handlers/mod.rs`. The `impl AppState` factory for that handler is
at the bottom of the same file.

## Call sites

Routes construct handlers via the factory:

```rust
// token_login.rs
let result = state.auth_handler()
    .authenticate_user(&state, ...)
    .await;
```

Not:

```rust
// WRONG
let result = AuthHandler::new(&state)
    .authenticate_user(&state, ...)
    .await;
```

## Summary of rules

| Rule | Reason |
|------|--------|
| Handlers borrow services, not `AppState` | Decouples handler from full state; makes dependencies explicit |
| Generic over `IDB`/`SDB` trait bounds | Keeps handlers testable and DB-agnostic in their own logic |
| Factory on `AppState` returns concrete types | Prevents opaque-type unification errors when composing handlers |
| Factory colocated in handler file | Easy to find; keeps construction logic next to the struct |
| `impl IdentityDb` banned in `impl AppState` factory returns | See concrete types row above |
