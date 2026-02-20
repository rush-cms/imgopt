FROM rust:1.85-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev libwebp-dev clang make nasm && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -f target/release/deps/imgopt*

COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libwebp7 curl && rm -rf /var/lib/apt/lists/*

# Run as non-root user
RUN groupadd -r imgopt && useradd -r -g imgopt imgopt

WORKDIR /app
COPY --from=builder /app/target/release/imgopt /usr/local/bin/imgopt

USER imgopt

ENV PORT=3000
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -fsS "http://localhost:${PORT:-3000}/health" > /dev/null || exit 1

CMD ["imgopt"]
