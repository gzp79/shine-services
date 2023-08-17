# Identity server

## VS code test

- start dev environment
- run stage:test
- run any test 


## Fly.io

To restart the postgres machine and server:
-  optionally `fly auth login`
-  `fly checks list -a shine-db`
-  `fly machine start ...`
-  `fly pg restart -a shine-db`

To proxy it to for local use and development:
- `fly proxy 15432:5432 -a shine-db`
