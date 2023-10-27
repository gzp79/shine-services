Write-Host "Setup buildx"
docker network create shine
docker buildx create --driver=docker-container --driver-opt=network=shine --use

cd service

Write-Host "Setup environment"
docker compose -f docker.yml -p shine-identity up -d
$pg_host=docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' shine-identity-postgres-1
$redis_host=docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' shine-identity-redis-1

Write-Host "Build action"
docker buildx build --target test --add-host postgres.mockbox.com:$pg_host --add-host redis.mockbox.com:$redis_host -t gzp79/shine-identity:test --progress=plain .

Write-Host "Start dockerized service"
docker compose -f docker.yml -p shine-identity --profile test up -d

Write-Host "Run tests"
cd ..\integration-test\
npm run jest regression



