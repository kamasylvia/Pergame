# Backend (Rust) Dockerfile for Pergame
#
# Runtime base must NOT be distroless/static: sqlx is built with
# `tls-native-tls`, so the binary dynamically links libssl/libcrypto and
# would fail at startup in a static image. distroless/cc also ships the CA
# bundle the S3/Postgres TLS handshakes need.
#
# Toolchain floor: toasty 0.11 requires rustc >= 1.95, so the builder image
# must not go below that (1.85 could not compile the dependency tree at all).
FROM rust:1.95-slim AS builder

WORKDIR /app

# Native build prerequisites for openssl-sys.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Manifest first so dependency compilation sits in its own cacheable layer.
# A stub main is enough: cargo resolves and builds the whole dependency graph.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Real sources; touch so cargo relinks the final binary.
COPY src ./src
COPY migrations ./migrations
COPY config.toml ./config.toml
COPY emails ./emails
RUN touch src/main.rs && cargo build --release

# Production stage — glibc + CA certs, non-root.
#
# Runtime base must match the builder's Debian release: rust:1.95-slim is
# trixie (glibc 2.41), so a bookworm runtime (glibc 2.36) fails at startup
# with "GLIBC_2.38/2.39 not found". ca-certificates is required for the
# S3/Postgres TLS handshakes.
FROM debian:trixie-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 65532 --create-home pergame

WORKDIR /app

COPY --from=builder --chown=pergame:pergame /app/target/release/pergame-rs ./

USER pergame

EXPOSE 8201

ENTRYPOINT ["./pergame-rs"]
