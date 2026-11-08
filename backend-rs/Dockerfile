# Backend (Rust) Dockerfile for Collectanea
FROM rust:1.85-slim AS builder

WORKDIR /app

# Manifest first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Stub sources so `cargo build --release` can compile dependencies,
# then overlay the real sources (cached dependency layer).
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release
COPY src ./src
COPY migrations ./migrations
COPY config.toml ./config.toml
COPY emails ./emails
RUN touch src/main.rs && cargo build --release

# Production stage — distroless
FROM gcr.io/distroless/static-debian12:nonroot

WORKDIR /app

COPY --from=builder --chown=nonroot:nonroot /app/target/release/collectanea-rs ./

EXPOSE 8201

ENTRYPOINT ["./collectanea-rs"]
