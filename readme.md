# Backend

## Environments

For web application to work properly on any environment, https is required. Depending on the selected environment, different tool provides the ssl.
In a similar way the DB also depends on the selected environment:

- local: Used for local development, independent of any cloud resources
  - DB: local
  - ssl: in the services, self-signed
- test: Used for integration test,  independent of any cloud resources
  - DB: local
  - ssl: nginx, self-signed
- dev: Allow to connect to cloud resources from the local machine, can be used debug attach to production DBs.
  - cloud hosted DB
  - ssl: in the services, self-signed
- prod: Production version
  - cloud hosted DB
  - ssl: hosting environment, managed by cloudflare, fly.io

### To start a local version (VS code)

- `start dev environment`
- `identity: local`


## Know-how 
### Fly.io

To restart the postgres machine and server:
-  optionally `fly auth login`
-  `fly checks list -a shine-db`
-  `fly machine start ...`

### Cargo extensions

These are the most frequently used cargo extensions in the shine project:

```shell
cargo install cargo-outdated
cargo install cargo-tree
cargo install trunk
```

### Telemetry

#### **Jaeger**

Set up telemetry configuration:
```json
  {
    "telemetry": {
      "tracing": {
        "type": "openTelemetryProtocol",
        "endpoint": "http://localhost:4317"
      }
    }
  }
```

Web view:
```shell
# Run jaeger in background with OTLP ingestion enabled.
$ docker run -d -p16686:16686 -p4317:4317 -e COLLECTOR_OTLP_ENABLED=true jaegertracing/all-in-one:latest

# View spans
$ firefox http://localhost:16686/
```
