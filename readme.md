# Identity server

## VS code test

- `start dev environment`
- `run stage:test`
- `test integration` or perform any other client operation


## Fly.io

**Deprecated DB has moved from fly.io only a single test instance of the service remained there**

To restart the postgres machine and server:
-  optionally `fly auth login`
-  `fly checks list -a shine-db`
-  `fly machine start ...`
-  `fly pg restart -a shine-db`

To proxy it to for local use and development:
- `fly proxy 15432:5432 -a shine-db`
 