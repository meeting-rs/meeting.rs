FROM rust:slim AS builder

RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libclang-dev \
    build-essential \
    curl

WORKDIR /app

COPY . .

RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh && \
    wasm-pack build -t web -d ../static/pkg peer && \
    cargo build -r -p coordinator

FROM debian:bullseye-slim

WORKDIR /app

RUN set -eux; \
      apt-get update; \
      apt-get install -y --no-install-recommends \
      ca-certificates

COPY --from=builder /app/target/release/coordinator /app/coordinator
COPY --from=builder /app/static /app/static

ENTRYPOINT [ "/app/coordinator" ]
