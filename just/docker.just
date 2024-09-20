
DOCKER_COMPOSE_DIR:="./btc-deploy"
DOCKER_COMPOSE_FILE:=DOCKER_COMPOSE_DIR + "/docker-compose.yml"

# Pull all the docker containers defined in the compose file
[group('docker')]
docker_pull: 
  docker compose -f "{{DOCKER_COMPOSE_FILE}}" pull


# Start all the docker containers defined in the compose file
[group('docker')]
docker_up: docker_down
  docker compose -f "{{DOCKER_COMPOSE_FILE}}" up -d --build
  sleep 2


# Stop all the docker containers defined in the compose file
[group('docker')]
docker_down:
  @# Stop services only:                     docker compose stop
  @# Stop and remove containers, networks..: docker compose down 
  @# Down and remove volumes:                docker compose down --volumes
  docker compose -f "{{DOCKER_COMPOSE_FILE}}" down --volumes
