FROM rust:1.75.0-bookworm as builder
WORKDIR /usr/src/
RUN git clone https://github.com/ordinals/ord
WORKDIR /usr/src/ord
RUN cargo build --bin ord --release

FROM debian:bookworm-slim
COPY --from=builder /usr/src/ord/target/release/ord /usr/local/bin
COPY ./scripts/test_ordinals_docker.sh test.sh
COPY ./brc20_json_artifacts brc20_json_artifacts/
RUN apt-get update && apt-get install -y snapd openssl jq
RUN snap install bitcoin-core
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info

ENTRYPOINT [ "sh", "test.sh" ]
