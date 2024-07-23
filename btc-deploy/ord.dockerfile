FROM ubuntu:22.04

WORKDIR /app
COPY ord.sh .
COPY mkcert .
RUN apt update && apt install -y curl
RUN curl -sL https://deb.nodesource.com/setup_22.x | bash
RUN apt install -y nodejs
RUN npm install -g local-ssl-proxy
RUN curl --proto '=https' --tlsv1.2 -fsLS https://raw.githubusercontent.com/ordinals/ord/master/install.sh | bash -s -- --to /app

ENV ORD_BITCOIN_RPC_USERNAME=ic-btc-integration
ENV ORD_BITCOIN_RPC_PASSWORD="QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E="

ENTRYPOINT ./ord.sh