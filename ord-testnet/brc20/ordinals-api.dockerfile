FROM node:18-alpine

WORKDIR /app
RUN apk add --no-cache --virtual .build-deps git

RUN git clone -b patch-genesis-block https://github.com/kobby-pentangeli/ordinals-api
# RUN git clone -b fix/ordhook-ingestion https://github.com/hirosystems/ordinals-api
# RUN git clone https://github.com/hirosystems/ordinals-api
WORKDIR /app/ordinals-api
RUN git fetch --all --tags

# Modify the `generate:git-info` command to create a `.git-info` file directly if the `npm run` command fails
RUN sed -i '/generate:git-info/c\"generate:git-info": "echo {} > .git-info",' package.json

RUN npm ci && npm run build

# Attempt to generate git info or skip if it fails
RUN npm run generate:git-info || echo "Skipping generate:git-info due to no tags"

RUN npm prune --production
RUN apk del .build-deps

ENTRYPOINT ["node", "./dist/src/index.js"]
