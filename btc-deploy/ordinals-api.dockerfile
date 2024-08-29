FROM ubuntu:22.04

WORKDIR /app
COPY ordinals-api.sh .
COPY mkcert .
RUN apt update && apt install -y curl
RUN curl -sL https://deb.nodesource.com/setup_22.x | bash
RUN apt install -y nodejs git
RUN npm install -g local-ssl-proxy

# Install ordinals-api
RUN git clone https://github.com/bitfinity-network/ordinals-api-regtest.git && \
  cd ordinals-api-regtest/ && \
  npm install && \
  npm run build && \
  npm run generate:git-info && \
  npm prune --production

EXPOSE 3000 3099 8005

ENTRYPOINT ./ordinals-api.sh
