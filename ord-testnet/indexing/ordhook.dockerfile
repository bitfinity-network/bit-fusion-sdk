FROM hirosystems/ordhook:latest

COPY ordhook.sh .
COPY mkcert .
COPY Ordhook.toml .
RUN apt update && apt install -y curl
RUN curl -sL https://deb.nodesource.com/setup_22.x | bash
RUN apt install -y nodejs
RUN npm install -g local-ssl-proxy

ENTRYPOINT ./ordhook.sh