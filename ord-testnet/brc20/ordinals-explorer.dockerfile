FROM node:latest

WORKDIR /usr/src

RUN apt-get update && apt-get install -y bash
RUN npm install -g local-ssl-proxy
RUN git clone https://github.com/hirosystems/ordinals-explorer

WORKDIR /usr/src/ordinals-explorer

RUN sed -i -e 's/${API_URL}/http:\/\/ordinals-api:3000/g' "app/(explorer)/inscription/[iid]/page.tsx"
RUN sed -i -e 's/https:\/\/api.hiro.so/http:\/\/localhost:3000/g' "lib/constants.ts"
RUN npm install
RUN npm run build

COPY ordinals-explorer.sh /usr/src/ordinals-explorer/
COPY mkcert /usr/src/ordinals-explorer/

RUN apt-get clean && rm -rf /var/lib/apt/lists/* 

ENTRYPOINT ["/usr/src/ordinals-explorer/ordinals-explorer.sh"]
