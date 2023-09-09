echo Setup buildx
docker network create shine
docker buildx create --driver=docker-container --driver-opt=network=shine --use

echo Setup environment
docker compose -f docker.yml up -d
set pg_host=docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' shine-identity-postgres-1
set redis_host=docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' shine-identity-redis-1

echo Build action
echo build is runing in the created buildx workflow (docker container) and for this 
docker buildx build --target test --add-host postgres.localhost.com:$pg_host --add-host redis.localhost.com:$redis_host --load -t gzp79/shine-identity:test --progress=plain .

echo Start dockerized service
docker compose -f docker.yml --profile test up -d

echo Run tests
cd .\integration-test\
npm run regression



