FROM node:18-alpine

WORKDIR /app
RUN apk add --no-cache --virtual .build-deps git
RUN git clone https://github.com/hirosystems/ordinals-api
WORKDIR /app/ordinals-api
RUN sed -i -e 's/BRC20_GENESIS_BLOCK = 779832/BRC20_GENESIS_BLOCK = 0/g' src/pg/brc20/brc20-pg-store.ts
RUN npm ci && \
    npm run build && \
    npm run generate:git-info && \
    npm prune --production
RUN apk del .build-deps

ENTRYPOINT ["node", "./dist/src/index.js"]
