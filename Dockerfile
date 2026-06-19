FROM rust:1.85-slim AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock* ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/RPS /usr/local/bin/RPS

COPY --from=builder /app/src/static ./src/static

EXPOSE 8000

CMD ["RPS"]