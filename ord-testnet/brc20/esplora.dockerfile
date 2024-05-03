FROM ubuntu:22.04

RUN apt update && apt install -y curl git && \
    curl -sL https://deb.nodesource.com/setup_22.x | bash - && \
    apt install -y nodejs && \
    rm -rf /var/lib/apt/lists/*
RUN npm install -g local-ssl-proxy

# Set up non-root user
RUN useradd -m esplorauser
USER esplorauser

WORKDIR /esplora
RUN git clone https://github.com/Blockstream/esplora.git .

COPY mkcert esplora.sh ./

RUN npm install && npm run dist

ENTRYPOINT ["./esplora.sh"]
