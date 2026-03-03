# Identity Service Refactoring Design

**Date**: 2026-03-03
**Status**: Approved
**Timeline**: 12-16 days

## Executive Summary

Refactor the identity service architecture to improve maintainability, reduce code duplication, and establish clear separation of concerns. The refactoring splits a monolithic `IdentityService` into four focused services, renames `controllers/` to `routes/` for clarity, extracts authentication logic into reusable handlers, and reduces auth page code by 50-80% through shared utilities.

**Key Changes**:
- Split `IdentityService` (24 methods, 5 concerns) → 4 focused services
- Rename `controllers/` → `routes/` for accurate terminology
- Create `AuthHandler` to extract 400+ lines from `token_login.rs`
- Add `AuthPageRequest` helper to eliminate auth page duplication
- Eliminate `CreateUserHandler` and `UserInfoHandler` (move to services)

**Benefits**:
- 35% reduction in total lines of code
- 50-80% reduction in auth page duplication
- Clear single responsibility for each service
- Easier testing (mock individual services)
- Consistent patterns across codebase

## Table of Contents

1. [Current Problems](#1-current-problems)
2. [Target Architecture](#2-target-architecture)
3. [Service Layer Design](#3-service-layer-design)
4. [Handler Layer Design](#4-handler-layer-design)
5. [Route Layer Design](#5-route-layer-design)
6. [Event Handling](#6-event-handling)
7. [Transaction Management](#7-transaction-management)
8. [Migration Strategy](#8-migration-strategy)
9. [Testing Strategy](#9-testing-strategy)
10. [Success Criteria](#10-success-criteria)

---

## 1. Current Problems

### 1.1 Unclear Layering

**Current: 4 layers with fuzzy boundaries**
```
Controllers → Handlers (unclear purpose) → Services (god object) → Repositories
```

**Issues**:
- Handler layer has inconsistent purpose:
  - `CreateUserHandler` is just a thin wrapper around `IdentityService.create_user()` with retry logic
  - `LoginEmailHandler` orchestrates user creation, token generation, and email sending
  - `UserInfoHandler` aggregates data from multiple services
- When to use a handler vs. calling services directly? No clear rule.

### 1.2 God Object Service

**IdentityService has 24 methods handling 5 different concerns:**

| Concern | Methods | Examples |
|---------|---------|----------|
| User CRUD | 8 | `create_user()`, `find_by_id()`, `update()`, `delete()` |
| External Links | 5 | `add_external_link()`, `find_by_external_link()` |
| Tokens | 8 | `add_token()`, `take_token()`, `test_token()` |
| Roles | 3 | `add_role()`, `get_roles()`, `delete_role()` |
| Search/Utils | 2 | `search()`, `generate_user_name()` |

**Issues**:
- Violates Single Responsibility Principle
- Hard to test (must mock entire service)
- Hard to understand (too many methods)
- Mixes infrastructure concerns (events, transactions) with domain logic

### 1.3 Auth Page Duplication

**Every auth page (email_login.rs, guest_login.rs, oauth2_login.rs, etc.) has:**
- ~25 lines: Query validation + redirect URL validation
- ~5 lines: Captcha validation
- ~10 lines: Auth session manipulation
- ~10 lines: Error handling

**Example: email_login.rs is 95 lines, only 45 are business logic**

### 1.4 Complex token_login.rs

**token_login.rs has 617 lines with ad-hoc functions:**
- `authenticate_with_query_token()` - 80+ lines
- `authenticate_with_header_token()` - 90+ lines
- `authenticate_with_cookie_token()` - 110+ lines
- `authenticate_with_refresh_session()` - 40+ lines
- `complete_email_login()` - 60+ lines
- Main `token_login()` handler - 120+ lines

**Issues**:
- Not reusable (defined as local functions)
- Hard to test (embedded in route handler)
- Hard to understand (400+ lines in one file)

### 1.5 Generic Boilerplate

**Every handler has repetitive generic boilerplate:**
```rust
pub struct CreateUserHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    identity_service: &'a IdentityService<IDB>,
}

impl<'a, IDB> CreateUserHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    pub fn new(identity_service: &'a IdentityService<IDB>) -> Self {
        Self { identity_service }
    }

    pub async fn create_user(&self, ...) -> Result<Identity, CreateUserError> {
        // 30 lines of retry logic
    }
}
```

For a stateless function with retry logic, this is excessive.

### 1.6 Naming Confusion

**Directory named "controllers/" but types named "AuthController":**
- In Axum, these are route handlers, not MVC controllers
- "Controller" suggests HTTP layer, but `controllers/` contains HTTP+orchestration
- Inconsistent with handler layer that also does "controlling"

---

## 2. Target Architecture

### 2.1 High-Level Overview

**Target: 3 layers with clear responsibilities**
```
Routes (HTTP + simple orchestration)
    ↓ borrows
Handlers (complex multi-service orchestration)
    ↓ borrows
Services (focused domain logic, owns repo)
    ↓ owns
Repositories (data abstraction)
```

### 2.2 Layering Rules

**Routes (HTTP Layer)**:
- HTTP request/response handling
- Input validation
- Simple orchestration (calling 1-2 services)
- **Can call**: Handlers OR Services directly
- **Cannot call**: Repositories directly

**Handlers (Orchestration Layer)**:
- Complex multi-service orchestration
- Workflows requiring 3+ services
- **Borrows**: Multiple services (doesn't own)
- **Cannot call**: Repositories directly (must go through services)

**Services (Domain Layer)**:
- Single domain responsibility
- Business logic for one concept (users, tokens, roles, links)
- **Owns**: Repository/DB
- **Cannot depend on**: Other services (flat structure)
- **Publishes**: Domain events via shared EventBus

**Repositories (Data Layer)**:
- Database abstraction
- External service abstraction
- Completely isolated from business logic

### 2.3 Directory Structure

```
src/
├─ routes/                           ← HTTP layer (renamed from controllers/)
│   ├─ auth/
│   │   ├─ pages/                    ← HTML endpoints
│   │   │   ├─ email_login.rs
│   │   │   ├─ guest_login.rs
│   │   │   ├─ token_login.rs
│   │   │   └─ ...
│   │   ├─ api/                      ← REST API endpoints
│   │   │   ├─ user_info.rs
│   │   │   ├─ sessions.rs
│   │   │   └─ ...
│   │   ├─ mod.rs
│   │   ├─ auth_page_request.rs      ← NEW: Reusable validation
│   │   └─ ...
│   ├─ identity/
│   └─ health/
│
├─ handlers/                         ← Orchestration layer
│   ├─ auth_handler.rs               ← NEW: Authentication flows
│   ├─ login_email_handler.rs        ← Refactored
│   └─ login_token_handler.rs        ← Refactored
│
├─ services/                         ← Domain layer
│   ├─ user_service.rs               ← NEW: Split from IdentityService
│   ├─ token_service.rs              ← NEW: Split from IdentityService
│   ├─ role_service.rs               ← NEW: Split from IdentityService
│   ├─ link_service.rs               ← NEW: Split from IdentityService
│   ├─ session_service.rs            ← Keep as-is
│   └─ events.rs                     ← NEW: Shared event bus
│
└─ repositories/                     ← Data layer (unchanged)
    ├─ identity/
    └─ session/
```

### 2.4 Terminology

| Term | Meaning | Example |
|------|---------|---------|
| **Route** | HTTP endpoint function | `email_login()` in routes/auth/pages/email_login.rs |
| **Router** | Struct aggregating routes | `AuthRouter` (was `AuthController`) |
| **Handler** | Multi-service orchestration | `AuthHandler`, `LoginEmailHandler` |
| **Service** | Domain logic, owns repo | `UserService`, `TokenService` |
| **Repository** | Data access abstraction | `PgIdentityDb`, `RedisSessionDb` |

---

## 3. Service Layer Design

### 3.1 Service Split

**Split IdentityService (24 methods) into 4 focused services:**

#### UserService (8 methods - Core identity operations)

```rust
// services/user_service.rs
pub struct UserService<DB: IdentityDb> {
    db: DB,
    name_generator: Box<dyn IdEncoder>,
    events: Arc<EventBus>,
}

impl<DB: IdentityDb> UserService<DB> {
    // CRUD operations
    pub async fn create(&self, id: Uuid, name: &str, email: Option<(&str, bool)>)
        -> Result<Identity, IdentityError>;
    pub async fn find_by_id(&self, id: Uuid)
        -> Result<Option<Identity>, IdentityError>;
    pub async fn find_by_email(&self, email: &str)
        -> Result<Option<Identity>, IdentityError>;
    pub async fn update(&self, id: Uuid, name: Option<&str>, email: Option<(&str, bool)>)
        -> Result<Option<Identity>, IdentityError>;
    pub async fn delete(&self, id: Uuid)
        -> Result<(), IdentityError>;

    // Search
    pub async fn search(&self, search: SearchIdentity<'_>)
        -> Result<Vec<Identity>, IdentityError>;

    // Name generation
    pub async fn generate_name(&self)
        -> Result<String, IdentityError>;

    // Complex workflow: create with retry (moved from CreateUserHandler)
    pub async fn create_with_retry(
        &self,
        name: Option<&str>,
        email: Option<&str>,
    ) -> Result<Identity, CreateUserError>;

    // Transactional: create + link (for OAuth flows)
    pub async fn create_linked_user(
        &self,
        user_id: Uuid,
        name: &str,
        external_user: &ExternalUserInfo,
    ) -> Result<Identity, IdentityError>;
}
```

**Maps to repository traits**: `Identities`, `IdentitySearch`, `IdSequences`

#### TokenService (8 methods - Token lifecycle)

```rust
// services/token_service.rs
pub struct TokenService<DB: IdentityDb> {
    db: DB,
    // No events - tokens are transient
}

impl<DB: IdentityDb> TokenService<DB> {
    // Create with retry logic
    pub async fn create_with_retry(
        &self,
        user_id: Uuid,
        kind: TokenKind,
        ttl: &Duration,
        fingerprint: Option<&ClientFingerprint>,
        email: Option<&str>,
        site_info: &SiteInfo,
    ) -> Result<String, TokenError>;  // Returns unhashed token

    // Query
    pub async fn find_by_hash(&self, token_hash: &str)
        -> Result<Option<TokenInfo>, IdentityError>;
    pub async fn list_by_user(&self, user_id: &Uuid)
        -> Result<Vec<TokenInfo>, IdentityError>;

    // Consume
    pub async fn test(&self, allowed_kinds: &[TokenKind], token: &str)
        -> Result<Option<(Identity, TokenInfo)>, IdentityError>;
    pub async fn take(&self, allowed_kinds: &[TokenKind], token: &str)
        -> Result<Option<(Identity, TokenInfo)>, IdentityError>;

    // Delete
    pub async fn delete(&self, kind: TokenKind, token: &str)
        -> Result<Option<()>, IdentityError>;
    pub async fn delete_by_user(&self, user_id: Uuid, token_hash: &str)
        -> Result<Option<()>, IdentityError>;
    pub async fn delete_all_by_user(&self, user_id: Uuid, kinds: &[TokenKind])
        -> Result<(), IdentityError>;
}
```

**Maps to repository trait**: `Tokens`

#### RoleService (3 methods - Authorization)

```rust
// services/role_service.rs
pub struct RoleService<DB: IdentityDb> {
    db: DB,
    events: Arc<EventBus>,
}

impl<DB: IdentityDb> RoleService<DB> {
    pub async fn add(&self, user_id: Uuid, role: &str)
        -> Result<Option<Vec<String>>, IdentityError>;
    pub async fn remove(&self, user_id: Uuid, role: &str)
        -> Result<Option<Vec<String>>, IdentityError>;
    pub async fn get(&self, user_id: Uuid)
        -> Result<Option<Vec<String>>, IdentityError>;
}
```

**Maps to repository trait**: `Roles`

#### LinkService (5 methods - External OAuth/OIDC)

```rust
// services/link_service.rs
pub struct LinkService<DB: IdentityDb> {
    db: DB,
    events: Arc<EventBus>,
}

impl<DB: IdentityDb> LinkService<DB> {
    pub async fn link(&self, user_id: Uuid, external_user: &ExternalUserInfo)
        -> Result<(), IdentityError>;
    pub async fn unlink(&self, user_id: Uuid, provider: &str, provider_id: &str)
        -> Result<Option<()>, IdentityError>;
    pub async fn find_by_provider(&self, provider: &str, provider_id: &str)
        -> Result<Option<Identity>, IdentityError>;
    pub async fn list_by_user(&self, user_id: Uuid)
        -> Result<Vec<ExternalLink>, IdentityError>;
    pub async fn is_linked(&self, user_id: Uuid)
        -> Result<bool, IdentityError>;
}
```

**Maps to repository trait**: `ExternalLinks`

### 3.2 Service Characteristics

**Each service:**
- ✅ Has single responsibility (mapped to one domain concept)
- ✅ Owns its repository/DB
- ✅ Independent of other services (no service-to-service calls)
- ✅ Publishes events (if domain changes need propagation)
- ✅ Maps to repository traits (1-to-1 or 1-to-few)

**Services do NOT:**
- ❌ Call other services
- ❌ Handle HTTP concerns
- ❌ Manage transactions across services (that's handler/route responsibility)

---

## 4. Handler Layer Design

### 4.1 Handler Characteristics

**Handlers are for:**
- Complex multi-service orchestration (3+ services)
- Workflows with retry/conditional logic
- Operations reused across multiple routes

**Handlers:**
- ✅ Borrow services (don't own)
- ✅ Coordinate multiple services
- ✅ Contain workflow logic
- ❌ Never own services
- ❌ Never depend on other handlers

### 4.2 New: AuthHandler

**Purpose**: Extract authentication logic from token_login.rs (400+ lines)

```rust
// handlers/auth_handler.rs
pub struct AuthHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    user_service: &'a UserService<IDB>,
    token_service: &'a TokenService<IDB>,
    role_service: &'a RoleService<IDB>,
    link_service: &'a LinkService<IDB>,
    session_service: &'a SessionService<SDB>,
}

pub struct AuthResult {
    pub identity: Identity,
    pub token_info: TokenInfo,
    pub create_access_token: bool,
    pub rotated_token: Option<String>,
}

impl<IDB, SDB> AuthHandler<'_, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    // Authentication methods (extracted from token_login.rs)
    pub async fn authenticate_query_token(
        &self,
        token: &str,
        fingerprint: &ClientFingerprint,
    ) -> Result<AuthResult, AuthError>;

    pub async fn authenticate_header_token(
        &self,
        token: &str,
        fingerprint: &ClientFingerprint,
    ) -> Result<AuthResult, AuthError>;

    pub async fn authenticate_cookie_token(
        &self,
        token: &str,
        user_id: Uuid,
        fingerprint: &ClientFingerprint,
    ) -> Result<AuthResult, AuthError>;

    pub async fn authenticate_session_refresh(
        &self,
        user_id: Uuid,
    ) -> Result<AuthResult, AuthError>;

    // Email verification (from complete_email_login)
    pub async fn complete_email_verification(
        &self,
        token_info: &TokenInfo,
        identity: &Identity,
        query_email_hash: Option<&str>,
    ) -> Result<Identity, AuthError>;

    // Session creation (from UserInfoHandler)
    pub async fn create_user_session(
        &self,
        identity: &Identity,
        fingerprint: &ClientFingerprint,
        site_info: &SiteInfo,
    ) -> Result<CurrentUser, UserInfoError>;

    // Session refresh (from UserInfoHandler)
    pub async fn refresh_all_sessions(
        &self,
        user_id: Uuid,
    ) -> Result<(), UserInfoError>;
}
```

**Replaces**:
- Ad-hoc functions in `token_login.rs`
- `UserInfoHandler.create_user_session()`
- `UserInfoHandler.refresh_user_session()`

### 4.3 Refactored: LoginEmailHandler

```rust
// handlers/login_email_handler.rs
pub struct LoginEmailHandler<'a, IDB, EMS>
where
    IDB: IdentityDb,
    EMS: EmailSender,
{
    random: &'a SystemRandom,
    settings_service: &'a SettingsService,
    user_service: &'a UserService<IDB>,      // Changed from identity_service
    token_service: &'a TokenService<IDB>,    // Changed from identity_service
    mailer_service: MailerService<'a, EMS>,
}

impl<IDB, EMS> LoginEmailHandler<'_, IDB, EMS> {
    pub async fn send_login_email(
        &self,
        email: &str,
        remember_me: Option<bool>,
        redirect_url: Option<&Url>,
        site_info: &SiteInfo,
        lang: Option<Language>,
    ) -> Result<Identity, LoginEmailError>;
}
```

**Changes**:
- Uses `UserService` instead of `IdentityService`
- Uses `TokenService` instead of `IdentityService`
- Logic remains same, dependencies clarified

### 4.4 Refactored: LoginTokenHandler

```rust
// handlers/login_token_handler.rs
pub struct LoginTokenHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    random: &'a SystemRandom,
    token_service: &'a TokenService<IDB>,    // Changed from identity_service
}

impl<IDB> LoginTokenHandler<'_, IDB> {
    pub async fn create_user_token(
        &self,
        user_id: Uuid,
        kind: TokenKind,
        time_to_live: &Duration,
        fingerprint: Option<&ClientFingerprint>,
        site_info: &SiteInfo,
    ) -> Result<UserToken, LoginTokenError>;
}
```

### 4.5 Deleted Handlers

**Delete: CreateUserHandler**
- Logic moved to `UserService.create_with_retry()`
- Eliminating unnecessary wrapper

**Delete: UserInfoHandler**
- Logic moved to `AuthHandler`
- Eliminating duplicate orchestration

### 4.6 AppState Handler Factories

```rust
// app_state.rs
impl AppState {
    // NEW handler
    pub fn auth_handler(&self) -> AuthHandler<impl IdentityDb, impl SessionDb> {
        AuthHandler::new(
            self.user_service(),
            self.token_service(),
            self.role_service(),
            self.link_service(),
            self.session_service(),
        )
    }

    // Refactored handlers
    pub fn login_email_handler(&self) -> LoginEmailHandler<impl IdentityDb, impl EmailSender> {
        LoginEmailHandler::new(
            self.random(),
            self.settings(),
            self.user_service(),
            self.token_service(),
            self.mailer_service(),
        )
    }

    pub fn login_token_handler(&self) -> LoginTokenHandler<impl IdentityDb> {
        LoginTokenHandler::new(
            self.random(),
            self.token_service(),
        )
    }
}
```

---

## 5. Route Layer Design

### 5.1 Directory Rename

**Rename: controllers/ → routes/**

**Rationale**:
- "Routes" more accurately describes Axum HTTP handlers
- Removes confusion with handler layer
- Standard Rust web framework terminology

**Type renames**:
- `AuthController` → `AuthRouter`
- `IdentityController` → `IdentityRouter`
- `HealthController` → `HealthRouter`

### 5.2 New: AuthPageRequest Helper

**Purpose**: Extract duplicated validation logic from auth pages

```rust
// routes/auth/auth_page_request.rs

/// Common query parameters for auth pages
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AuthPageQuery {
    pub redirect_url: Option<Url>,
    pub error_url: Option<Url>,
    pub captcha: Option<String>,
}

/// Helper for auth page request handling
pub struct AuthPageRequest<'a> {
    state: &'a AppState,
    auth_session: AuthSession,
}

impl<'a> AuthPageRequest<'a> {
    pub fn new(state: &'a AppState, auth_session: AuthSession) -> Self {
        Self { state, auth_session }
    }

    /// Validate query (includes redirect URL validation)
    pub async fn validate_query<T>(
        &self,
        query: Result<ValidatedQuery<T>, ErrorResponse<InputError>>,
    ) -> Result<T, AuthPage>
    where
        T: HasAuthPageQuery;

    /// Validate captcha
    pub async fn validate_captcha(
        &self,
        captcha: Option<&str>,
        error_url: Option<&Url>
    ) -> Result<(), AuthPage>;

    /// Clear auth state (revoke session + access)
    pub async fn clear_auth_state(mut self) -> Self;

    /// Get auth session
    pub fn auth_session(&self) -> &AuthSession;

    /// Consume and return auth session
    pub fn into_auth_session(self) -> AuthSession;

    /// Error page helper
    pub fn error_page<E>(&self, error: E, error_url: Option<&Url>) -> AuthPage
    where
        E: Into<Problem>;
}

/// Trait for query types that contain common auth page fields
pub trait HasAuthPageQuery {
    fn auth_query(&self) -> &AuthPageQuery;
}
```

### 5.3 Auth Page Pattern

**Every auth page follows this pattern:**

```rust
#[derive(Debug, Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PageQuery {
    #[serde(flatten)]
    auth: AuthPageQuery,       // Common fields
    // ... page-specific fields
}

impl HasAuthPageQuery for PageQuery {
    fn auth_query(&self) -> &AuthPageQuery {
        &self.auth
    }
}

pub async fn page_handler(
    State(state): State<AppState>,
    query: Result<ValidatedQuery<PageQuery>, ErrorResponse<InputError>>,
    auth_session: AuthSession,
    // ... other extractors
) -> AuthPage {
    // 1. Create request helper
    let req = AuthPageRequest::new(&state, auth_session);

    // 2. Validate query (includes redirect URL validation)
    let query = req.validate_query(query).await?;

    // 3. Validate captcha (if needed)
    req.validate_captcha(query.auth.captcha.as_deref(), query.auth.error_url.as_ref()).await?;

    // 4. Clear/manipulate auth state (if needed)
    let req = req.clear_auth_state().await;

    // 5. Business logic (handlers/services)
    let result = state.some_handler()
        .do_work(...)
        .await
        .map_err(|err| req.error_page(err, query.auth.error_url.as_ref()))?;

    // 6. Return response
    PageUtils::new(&state).redirect(
        req.into_auth_session(),
        query.auth.redirect_url.as_ref(),
        None
    )
}
```

### 5.4 Code Reduction Examples

**email_login.rs:**
- Before: 95 lines (50 boilerplate, 45 business logic)
- After: 40 lines (10 setup, 30 business logic)
- **Reduction: 58%**

**token_login.rs:**
- Before: 617 lines (400 authentication functions, 217 handler)
- After: 120 lines (all in handler, authentication moved to AuthHandler)
- **Reduction: 80%**

**guest_login.rs:**
- Before: 131 lines
- After: ~60 lines
- **Reduction: 54%**

---

## 6. Event Handling

### 6.1 Shared EventBus

**Purpose**: Decouple event publishing from services

```rust
// services/events.rs
#[derive(Clone)]
pub struct EventBus {
    bus: TopicBus<IdentityTopic>,
}

impl EventBus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { bus: TopicBus::new() })
    }

    // User events
    pub async fn user_created(&self, user_id: Uuid);
    pub async fn user_updated(&self, user_id: Uuid);
    pub async fn user_deleted(&self, user_id: Uuid);

    // Role events (triggers session refresh)
    pub async fn role_changed(&self, user_id: Uuid);

    // Link events (triggers session refresh)
    pub async fn user_linked(&self, user_id: Uuid);
    pub async fn user_unlinked(&self, user_id: Uuid);

    // Subscribe
    pub async fn subscribe<E, H>(&self, handler: H) -> EventHandlerId
    where
        E: TopicEvent<Topic = IdentityTopic>,
        H: EventHandler<E>;
}
```

### 6.2 Event Flow

**Services publish events:**
```rust
impl<DB: IdentityDb> UserService<DB> {
    pub async fn create(&self, ...) -> Result<Identity, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        let identity = ctx.create_user(...).await?;
        drop(ctx);  // Commit

        self.events.user_created(identity.id).await;  // Publish
        Ok(identity)
    }
}
```

**SessionService subscribes to refresh sessions:**
```rust
// On startup
let events = state.events();
let session_service = state.session_service();

events.subscribe(|event: UserEvent| async move {
    match event {
        UserEvent::RoleChange(user_id) => {
            // Refresh all sessions for this user
            session_service.refresh_all(user_id).await;
        }
        _ => {}
    }
}).await;
```

**Pattern**:
- Services publish, don't subscribe (keeps them simple)
- Subscribers are registered at app startup
- Events are async, non-blocking

---

## 7. Transaction Management

### 7.1 Two Patterns

**Pattern A: Controller manages transaction** (for flexibility)

```rust
pub async fn oauth_callback(
    State(state): State<AppState>,
) -> Result<Json<CurrentUser>, ProblemResponse> {
    // 1. Get shared DB context
    let mut ctx = state.db().create_context().await?;

    // 2. Multiple operations in one transaction
    let user_id = Uuid::new_v4();
    let name = state.user_service().generate_name().await?;
    let identity = ctx.create_user(user_id, &name, None).await?;
    ctx.link_user(user_id, &external_user).await?;

    drop(ctx);  // Commit transaction

    // 3. Publish events after successful transaction
    state.events().user_created(user_id).await;
    state.events().user_linked(user_id).await;

    Ok(Json(CurrentUser { identity }))
}
```

**Use when**:
- Need fine-grained transaction control
- Operation is route-specific
- Uncommon workflow

**Pattern B: Service provides atomic operation** (for common patterns)

```rust
impl<DB: IdentityDb> UserService<DB> {
    pub async fn create_linked_user(
        &self,
        user_id: Uuid,
        name: &str,
        external_user: &ExternalUserInfo,
    ) -> Result<Identity, IdentityError> {
        let mut ctx = self.db.create_context().await?;

        let identity = ctx.create_user(user_id, name, None).await?;

        // Rollback on link failure
        if let Err(err) = ctx.link_user(user_id, external_user).await {
            let _ = ctx.cascaded_delete(user_id).await;
            return Err(err);
        }

        drop(ctx);  // Commit

        self.events.user_created(user_id).await;
        self.events.user_linked(user_id).await;

        Ok(identity)
    }
}
```

**Use when**:
- Workflow is common (e.g., OAuth login always creates + links)
- Operation makes sense as atomic unit
- Want to hide transaction complexity from routes

### 7.2 Recommendation

**Default to Pattern A** (controller manages):
- More flexible
- Explicit about transaction boundaries
- Routes understand what's atomic

**Use Pattern B** for frequently-used combinations:
- `create_linked_user()` - OAuth login flow
- Other common atomic operations

---

## 8. Migration Strategy

### 8.1 Overview

**Goal**: Incremental migration with working code at each step

**Phases**:
1. Preparation (1-2 days) - Setup structure
2. Service Split (3-4 days) - Create new services alongside old
3. Handler Refactoring (3-4 days) - Migrate handlers to new services
4. Route Refactoring (4-5 days) - Migrate routes to new handlers
5. Cleanup (1 day) - Delete old code

**Total: 12-16 days**

### 8.2 Phase 1: Preparation (1-2 days)

**Goal**: Setup new structure without breaking existing code

**Steps**:
1. Rename controllers/ → routes/
   - Update all imports
   - Rename types: AuthController → AuthRouter
   - Run tests

2. Create new service files (empty)
   - services/user_service.rs
   - services/token_service.rs
   - services/role_service.rs
   - services/link_service.rs
   - services/events.rs

3. Create new handler files (empty)
   - handlers/auth_handler.rs

4. Create auth page helper (empty)
   - routes/auth/auth_page_request.rs

**Validation**: All tests pass

### 8.3 Phase 2: Service Split (3-4 days)

**Goal**: Create new services alongside old IdentityService

**Steps**:
1. Implement EventBus (services/events.rs)
2. Implement UserService (services/user_service.rs)
3. Implement TokenService (services/token_service.rs)
4. Implement RoleService (services/role_service.rs)
5. Implement LinkService (services/link_service.rs)
6. Update AppState (dual services - keep old, add new)

**AppState during migration**:
```rust
struct Inner {
    // OLD - keep for compatibility
    identity_service: IdentityService<PgIdentityDb>,

    // NEW - add new services
    events: Arc<EventBus>,
    user_service: UserService<PgIdentityDb>,
    token_service: TokenService<PgIdentityDb>,
    role_service: RoleService<PgIdentityDb>,
    link_service: LinkService<PgIdentityDb>,
}

impl AppState {
    #[deprecated(note = "Use user_service(), token_service(), etc.")]
    pub fn identity_service(&self) -> &IdentityService<impl IdentityDb>;

    pub fn user_service(&self) -> &UserService<impl IdentityDb>;
    pub fn token_service(&self) -> &TokenService<impl IdentityDb>;
    pub fn role_service(&self) -> &RoleService<impl IdentityDb>;
    pub fn link_service(&self) -> &LinkService<impl IdentityDb>;
}
```

**Validation**:
- All tests pass (using old service)
- New services have unit tests

### 8.4 Phase 3: Handler Refactoring (3-4 days)

**Goal**: Migrate handlers to use new services

**Steps**:
1. Implement AuthHandler (handlers/auth_handler.rs)
2. Refactor LoginEmailHandler (use UserService + TokenService)
3. Refactor LoginTokenHandler (use TokenService)
4. Delete CreateUserHandler (logic in UserService.create_with_retry)
5. Delete UserInfoHandler (logic in AuthHandler)
6. Update AppState handler factories

**Validation**:
- Handler unit tests pass
- Integration tests still work

### 8.5 Phase 4: Route Refactoring (4-5 days)

**Goal**: Migrate routes to use new handlers and helpers

**Steps**:
1. Implement AuthPageRequest helper
2. Refactor token_login.rs (use AuthHandler)
3. Refactor email_login.rs (use AuthPageRequest + LoginEmailHandler)
4. Refactor guest_login.rs (use AuthPageRequest + UserService)
5. Refactor OAuth/OIDC login pages
6. Refactor other auth pages
7. Refactor API routes

**Migration order** (by risk):
1. guest_login.rs (simplest)
2. email_login.rs
3. oauth2_login.rs, oidc_login.rs
4. token_login.rs (most complex)
5. API routes

**Validation**:
- Route tests pass
- Integration tests pass
- Manual testing of auth flows

### 8.6 Phase 5: Cleanup (1 day)

**Goal**: Remove deprecated code

**Steps**:
1. Search for identity_service() usage (ensure none remain)
2. Delete services/identity_service.rs
3. Remove identity_service from AppState
4. Update documentation
5. Final validation

**Validation**:
- All tests pass
- No compiler warnings
- No dead code

### 8.7 Rollback Strategy

**If issues found**:

**Phase 1-2**:
- Revert git commits
- No breaking changes yet

**Phase 3**:
- Keep old handlers alongside new
- Routes still use old handlers

**Phase 4**:
- Migrate routes one by one
- Can revert individual routes

**Phase 5**:
- Keep IdentityService in deprecated/ folder temporarily

---

## 9. Testing Strategy

### 9.1 Testing Per Phase

**Phase 1 (Preparation)**:
- ✅ Compilation check
- ✅ All existing tests pass

**Phase 2 (Service Split)**:
- ✅ Unit tests for each new service
- ✅ Mock repository layer
- ✅ Test event publishing
- ✅ Integration tests still use old service

**Phase 3 (Handler Refactoring)**:
- ✅ Unit tests for each handler
- ✅ Mock new services
- ✅ Test all authentication flows in AuthHandler
- ✅ Integration tests still pass

**Phase 4 (Route Refactoring)**:
- ✅ Update route tests
- ✅ Playwright API tests (already exist)
- ✅ Manual testing of auth flows:
  - Email login (register + login)
  - Guest login
  - OAuth2 login
  - OIDC login
  - Token login (all paths: query, header, cookie, session)
  - Session refresh

**Phase 5 (Cleanup)**:
- ✅ Final full test suite run
- ✅ No dead code warnings

### 9.2 Test Coverage Goals

**Unit tests**:
- Each service: 80%+ coverage
- Each handler: 80%+ coverage
- AuthPageRequest helper: 90%+ coverage

**Integration tests**:
- All auth flows: 100% coverage (already have Playwright tests)
- API endpoints: 100% coverage

### 9.3 Manual Testing Checklist

**Before Phase 5 (cleanup)**:

- [ ] Email registration flow (new user)
- [ ] Email login flow (existing user)
- [ ] Guest registration
- [ ] OAuth2 login (Google/GitHub)
- [ ] OIDC login
- [ ] Token login with query parameter
- [ ] Token login with Authorization header
- [ ] Token login with cookie
- [ ] Session refresh without access token
- [ ] Email verification
- [ ] Role changes trigger session refresh
- [ ] External link/unlink triggers session refresh
- [ ] Token rotation works correctly
- [ ] Fingerprint validation works

---

## 10. Success Criteria

### 10.1 Technical Criteria

**Code Quality**:
- [ ] 35% reduction in total lines of code
- [ ] 50-80% reduction in auth page duplication
- [ ] No compiler warnings
- [ ] No `#[allow(dead_code)]` needed

**Testing**:
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] All Playwright tests pass
- [ ] Code coverage maintained or improved

**Performance**:
- [ ] No performance degradation
- [ ] Auth flow latency unchanged
- [ ] Database query count unchanged

### 10.2 Architectural Criteria

**Layering**:
- [ ] Services have single responsibility
- [ ] No service-to-service dependencies
- [ ] Handlers only borrow services
- [ ] Routes follow consistent pattern
- [ ] Clear separation: routes → handlers → services → repositories

**Naming**:
- [ ] Consistent terminology throughout
- [ ] No confusion between routes/handlers/services
- [ ] Clear module boundaries

### 10.3 Documentation Criteria

- [ ] CLAUDE.md updated with new architecture
- [ ] This design doc complete
- [ ] Inline comments updated
- [ ] Migration completed without issues

---

## Appendix A: File Changes Summary

### A.1 Renamed Files

```
controllers/              → routes/
controllers/mod.rs        → routes/mod.rs
controllers/auth/         → routes/auth/
controllers/identity/     → routes/identity/
controllers/health/       → routes/health/
```

### A.2 New Files

```
services/user_service.rs          (split from identity_service.rs)
services/token_service.rs         (split from identity_service.rs)
services/role_service.rs          (split from identity_service.rs)
services/link_service.rs          (split from identity_service.rs)
services/events.rs                (extracted from identity_service.rs)
handlers/auth_handler.rs          (extracted from routes/auth/pages/token_login.rs)
routes/auth/auth_page_request.rs  (new helper for validation)
```

### A.3 Deleted Files

```
services/identity_service.rs      (split into 4 services)
handlers/create_user_handler.rs   (moved to UserService)
handlers/user_info_handler.rs     (moved to AuthHandler)
```

### A.4 Modified Files

```
handlers/login_email_handler.rs   (use new services)
handlers/login_token_handler.rs   (use new services)
routes/auth/pages/email_login.rs  (use AuthPageRequest)
routes/auth/pages/guest_login.rs  (use AuthPageRequest)
routes/auth/pages/token_login.rs  (use AuthHandler)
routes/auth/pages/oauth2_login.rs (use AuthPageRequest)
routes/auth/pages/oidc_login.rs   (use AuthPageRequest)
routes/auth/api/user_info.rs      (use new services)
routes/auth/api/tokens.rs         (use TokenService)
routes/identity/api/*.rs          (use new services)
app_state.rs                      (expose new services)
main.rs                           (use AuthRouter instead of AuthController)
```

---

## Appendix B: Code Metrics

### B.1 Lines of Code Estimate

| Component | Before | After | Change |
|-----------|--------|-------|--------|
| IdentityService | 280 | 0 | -280 |
| New services (4) | 0 | 400 | +400 |
| Handlers | 350 | 450 | +100 |
| Routes | 2500 | 1200 | -1300 |
| **Total** | **3130** | **2050** | **-1080 (-35%)** |

### B.2 Service Method Distribution

| Service | Methods | Lines | Responsibility |
|---------|---------|-------|----------------|
| UserService | 8 | ~150 | User CRUD, search, name generation |
| TokenService | 8 | ~120 | Token lifecycle |
| RoleService | 3 | ~60 | Role management |
| LinkService | 5 | ~70 | External OAuth/OIDC linking |
| **Total** | **24** | **~400** | Was 1 service, now 4 |

---

## Appendix C: Dependencies

### C.1 Service Dependencies

```
UserService → PgIdentityDb, EventBus
TokenService → PgIdentityDb
RoleService → PgIdentityDb, EventBus
LinkService → PgIdentityDb, EventBus
SessionService → RedisSessionDb
```

### C.2 Handler Dependencies

```
AuthHandler → UserService, TokenService, RoleService, LinkService, SessionService
LoginEmailHandler → UserService, TokenService, MailerService
LoginTokenHandler → TokenService
```

### C.3 Route Dependencies

```
Routes → Handlers and/or Services (never Repositories directly)
```

---

## Conclusion

This refactoring establishes clear architectural boundaries, eliminates code duplication, and improves maintainability. The incremental migration strategy ensures the system remains working at each step, with clear rollback points if issues arise.

**Key improvements**:
- 35% less code
- 50-80% less duplication
- Clear single responsibility
- Easy to test and maintain
- Consistent patterns throughout

The architecture follows industry best practices while being pragmatic about transaction management and event handling.
