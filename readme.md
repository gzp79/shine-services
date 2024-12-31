# Backend

### To start a local version (VS code)

- `start dev environment`
- `identity: local`

## Know-how 

### How to fix 'failed to run custom build command for `aws-lc-sys`' on Windows

- rustls requires some other dependencies and it may result in `aws-lc-sys` compile errors
  - <https://medium.com/@rrnazario/rust-how-to-fix-failed-to-run-custom-build-command-for-aws-lc-sys-on-windows-c3bd2405ac6f>
  - <https://github.com/rustls/rustls/issues/1913>

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
