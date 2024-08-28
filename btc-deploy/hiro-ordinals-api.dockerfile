FROM ubuntu:22.04

WORKDIR /app
COPY ordinals-api.sh .
COPY mkcert .
RUN apt update && apt install -y curl
RUN curl -sL https://deb.nodesource.com/setup_22.x | bash
RUN apt install -y nodejs git
RUN npm install -g local-ssl-proxy

# Install ordinals-api
RUN git clone https://github.com/hirosystems/ordinals-api.git && \
  cd ordinals-api && \
  git checkout v4.0.4 && \
  npm install && \
  npm run build && \
  npm run generate:git-info && \
  npm prune --production && \
  sed -i -e 's/BRC20_GENESIS_BLOCK = 779832/BRC20_GENESIS_BLOCK = 0/g' src/pg/brc20/brc20-pg-store.ts && \
  sed -i -e 's/ORDINALS_GENESIS_BLOCK = 767430/ORDINALS_GENESIS_BLOCK = 0/g' src/pg/pg-store.ts


EXPOSE 3000 3099 8005

ENTRYPOINT ./ordinals-api.sh
