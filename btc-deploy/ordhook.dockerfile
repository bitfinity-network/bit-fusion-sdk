FROM rust:bullseye as build

ARG GIT_COMMIT='ee160a28'

ENV GIT_COMMIT=${GIT_COMMIT}

WORKDIR /src

RUN apt-get update && apt-get install -y git ca-certificates pkg-config libssl-dev libclang-11-dev libunwind-dev libunwind8 curl gnupg

RUN rustup update 1.77.1 && rustup default 1.77.1

RUN git clone https://github.com/veeso/ordhook.git /src && cd /src

RUN mkdir /out

ENV NODE_MAJOR=18

RUN mkdir -p /etc/apt/keyrings

RUN curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg

RUN echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_$NODE_MAJOR.x nodistro main" | tee /etc/apt/sources.list.d/nodesource.list

RUN apt-get update

RUN apt-get install nodejs -y

RUN npm install -g @napi-rs/cli yarn

WORKDIR /src/components/ordhook-cli

RUN cargo build --features release --release

RUN cp /src/target/release/ordhook /out

FROM debian:bullseye-slim

WORKDIR /ordhook-sdk-js

RUN apt-get update && apt-get install -y ca-certificates libssl-dev libclang-11-dev libunwind-dev libunwind8 sqlite3

COPY --from=build /out/ordhook /bin/ordhook

COPY ordhook.sh /bin/ordhook.sh

WORKDIR /workspace

ENTRYPOINT ["ordhook.sh"]