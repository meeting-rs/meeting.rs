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
    curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin && \
    just build

FROM gcr.io/distroless/cc

WORKDIR /app

COPY --from=builder /app/target/release/coordinator /app/coordinator
COPY --from=builder /app/static /app/static

ENTRYPOINT [ "/app/coordinator" ]
