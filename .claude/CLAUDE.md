# Shine Services

## Project Structure
- **Language**: Rust (edition 2021)
- **Type**: Workspace with multiple crates and services
- **Crates**: `shine-test-macros`, `shine-test`, `shine-core`, `shine-infra-macros`, `shine-infra`
- **Services**: `identity`, `builder`
## Testing
- **API Tests**: Playwright with `@playwright/test` (request context only, no browser pages)
- **Location**: `tests/` directory
- **Config**: `tests/playwright.config.ts`

## Key Dependencies
- Web framework: Axum
- Database: PostgreSQL (tokio-postgres), Redis (bb8-redis)
- Async runtime: Tokio
- Serialization: Serde (JSON, MessagePack)
- API docs: Utoipa (OpenAPI/Swagger)
