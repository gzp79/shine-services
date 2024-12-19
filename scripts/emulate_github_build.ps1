Write-Host "Setup buildx"
docker network create shine
docker buildx create --name shine-build --driver=docker-container --driver-opt=network=shine --use

Write-Host "Setup environment"
docker compose -f docker-compose.yml -p shine up -d
$pg_host=docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' shine-postgres-1
$redis_host=docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' shine-redis-1
Write-Host "  pg: $pg_host"
Write-Host "  redis: $redis_host"

Write-Host "Build action"
# --load option is added only for local test to cache the layers for the next build
docker buildx build --target test --add-host postgres.mockbox.com:$pg_host --add-host redis.mockbox.com:$redis_host -t gzp79/shine:test --load .

Write-Host "Start dockerized service"
docker compose -f docker-compose.yml -p shine --profile test up 

#Write-Host "Wait for services"
#sleep 3

#Write-Host "Run tests"
#cd tests\
#npm run jest regression



