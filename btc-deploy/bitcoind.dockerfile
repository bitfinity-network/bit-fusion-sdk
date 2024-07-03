FROM ubuntu:22.04

COPY bitcoin.conf /app/bitcoin.conf

RUN apt update && apt install -y curl wget build-essential libssl-dev && \
  wget -O /tmp/bitcoin.tar.gz https://bitcoin.org/bin/bitcoin-core-27.0/bitcoin-27.0-x86_64-linux-gnu.tar.gz && \
  mkdir -p /app/bitcoin && \
  mkdir -p /app/data && \
  tar -xzf /tmp/bitcoin.tar.gz -C /app && \
  cd /app/bitcoin-27.0/bin && \
  ln -s /app/bitcoin-27.0/bin/bitcoind /usr/bin/bitcoind

EXPOSE 18443 18444 28332 28333

CMD ["bitcoind", "-conf=/app/bitcoin.conf", "-datadir=/app/data", "-printtoconsole", "-regtest=1", "-rpcallowip=0.0.0.0/0", "-rpcbind=0.0.0.0", "-txindex"]
