# Identity server

## Fly.io

To restart the postgres machine and server:
-  optionally `fly auth login`
-  `fly checks list -a shine-db`
-  `fly machine start ...`
-  `fly pg restart -a shine-db`

To proxy it to use locally (without static IP, ingress services can be access only through proxy)
- `fly proxy 15432:5432 -a shine-db`
