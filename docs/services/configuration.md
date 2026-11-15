# Service Configuration Strategy

This document describes how services are configured across different deployment environments and how ports are managed.

## Environment Configurations

| Scenario | Environment | Service Ports | Client Access Port | Docker | Override |
|----------|-------------|---------------|-------------------|--------|----------|
| **Local dev** | `test`/`local` | 8443/8444 | 8443/8444 | ❌ | `SHINE__SERVICE__PORT` in task.json |
| **Docker (CI/CD)** | `test` | 7000/7001 | 443/80 | ✅ | None (default) |
| **Prod (Docker)** | `prod` | 7000/7001 | 443/80 | ✅ | None (default) |

## Key Points

- **Local development**: Services listen on **8443** (identity) and **8444** (builder). Clients connect directly to these ports. No nginx, no Docker.
- **Docker deployment**: Services listen on **7000** (identity) and **7001** (builder) internally. Nginx handles external client connections on standard **443/80** and routes them internally to the services.
- **Single config file**: `server_config.test.json` defaults to Docker ports (7000/7001), with local dev overriding via environment variables.

## Configuration Layer System

Each service uses a layered configuration approach:

```json
{
  "beforeLayers": [
    "file://../config/common.mock.json",      // Shared mock/test baseline
    "file://../config/common.test.json",       // Test environment specifics
    "file://config/mock.json"                  // Service-specific test settings
  ],
  "afterLayers": ["environment"]               // Environment variables override all
}
```

The `"afterLayers": ["environment"]` means any environment variables prefixed with `SHINE__` will override the configuration.

## Mock Configuration

Test environments use mocked external services via the config layer system:

- **Identity**: `config/mock.json` — mocks OAuth2, OIDC, SMTP providers
- **Builder**: `config/mock.json` — mocks hub communication, game services

When running tests locally (`test` config stage), these mocks are automatically loaded and provide:
- **SMTP**: localhost:2525 (mail capture)
- **OAuth2**: localhost:8090 (identity provider mock)
- **OIDC**: localhost:8091 (OpenID Connect mock)

Mock services are separate from the main application and managed by the test runner.

## Local Development

For detailed instructions on starting services, running tests, and troubleshooting, see the [local-development skill](/.claude/skills/local-development/SKILL.md).

To override service ports for local runs (when not using Docker):
```bash
# Set environment variable before running service (use double-dash separator)
export SHINE--SERVICE--PORT=8443  # Identity
export SHINE--SERVICE--PORT=8444  # Builder
```

Or use VSCode tasks which handle this automatically.
