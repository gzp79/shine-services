# Shine Services

## Project Structure
- **Language**: Rust (edition 2021)
- **Type**: Workspace with multiple crates and services
- **Crates**: `shine-test-macros`, `shine-test`, `shine-core`, `shine-infra-macros`, `shine-infra`
- **Services**: `identity`, `builder`

## Local Development
- **Run Identity Service**: Use VSCode task "identity: local" or run script from `services/identity/`: `./run_identity_local.ps1` (PowerShell) or `./run_identity_local.sh` (bash)
- **Run Tests**: `cd tests && pnpm test:local` (requires service running on port 8443)
- **Service expects**: HTTPS on port 8443, config at `services/identity/server_config.test.json`

## Testing
- **API Tests**: Playwright with `@playwright/test` (request context only, no browser pages)
- **Location**: `tests/` directory
- **Config**: `tests/playwright.config.ts`
- **Test env**: Service at `https://cloud.local.scytta.com:8443/identity`

## Key Dependencies
- Web framework: Axum
- Database: PostgreSQL (tokio-postgres), Redis (bb8-redis)
- Async runtime: Tokio
- Serialization: Serde (JSON, MessagePack)
- API docs: Utoipa (OpenAPI/Swagger)

## Architecture Patterns

### Service Layer
- **Services** (`services/`): Business logic, orchestrate repositories and handlers
- **Handlers** (`handlers/`): High-level operations, use multiple services
- **Routes** (`routes/`): HTTP endpoints, use handlers, minimal logic

### Auth Pages Pattern
- Use `AuthPageRequest` helper for validation (routes/auth/auth_page_request.rs)
- Standard flow: validate query → validate redirects → validate captcha → clear auth state → business logic → redirect
- Early-return with `Option<AuthPage>` for validation failures

### Email Handling
- External logins (OAuth2/OIDC) must validate and store emails from providers
- Use `email.validate_email()` before storing to filter invalid addresses
- Email storage enables email-based login for linked users

## Best Practices
- Use dedicated Read/Edit/Write tools, not bash for file operations
- Validate emails before storage (check `validate_email()` trait)
- Auth page handlers should follow consistent validation pattern
- Test failures after refactoring may indicate pre-existing bugs, not refactoring issues
- On Windows, use PowerShell for env vars with `--` in names (bash doesn't support)
