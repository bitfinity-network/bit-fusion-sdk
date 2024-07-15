FROM node:lts-slim

RUN npm install -g local-ssl-proxy

COPY ./mkcert /cert

EXPOSE 8002

CMD ["local-ssl-proxy", "--source", "8002", "--target", "8545", "--hostname", "host.docker.internal", "--key", "/cert/localhost+3-key.pem", "--cert", "/cert/localhost+3.pem"]
