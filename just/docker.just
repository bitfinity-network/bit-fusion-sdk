
DOCKER_COMPOSE_DIR:="./btc-deploy"
DOCKER_COMPOSE_FILE:=DOCKER_COMPOSE_DIR + "/docker-compose.yml"

# Start all the docker containers defined in the compose file
[group('docker')]
docker_start: docker_stop
  docker compose -f "{{DOCKER_COMPOSE_FILE}}" up -d --build
  sleep 2


# Stop all the docker containers defined in the compose file
[group('docker')]
docker_stop:
  docker compose -f "{{DOCKER_COMPOSE_FILE}}" down
