echo Setup buildx
docker buildx use default

echo Setup environment
docker compose -f docker.yml up -d

echo Build action
docker buildx build --target test --add-host postgres.localhost.com:host-gateway --add-host redis.localhost.com:host-gateway --load -t gzp79/shine-identity:test --progress=plain .

echo Start dockerized service
docker compose -f docker.yml --profile test up -d

echo Run tests
cd .\integration-test\
npm run regression



