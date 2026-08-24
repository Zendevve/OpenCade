# syntax=docker/dockerfile:1

# --- builder ---------------------------------------------------------------
FROM rust:1.98-bookworm AS builder
WORKDIR /app

# Copy workspace manifests first for layer caching.
# The lockfile is part of the MSRV and reproducibility contract.
COPY Cargo.toml Cargo.lock ./
COPY apps/server/Cargo.toml apps/server/Cargo.toml
COPY packages/protocol/Cargo.toml packages/protocol/Cargo.toml
COPY packages/networking/Cargo.toml packages/networking/Cargo.toml
COPY packages/emulator-sdk/Cargo.toml packages/emulator-sdk/Cargo.toml
COPY packages/shared/Cargo.toml packages/shared/Cargo.toml
COPY packages/game-definitions/Cargo.toml packages/game-definitions/Cargo.toml
COPY adapters/fbneo/Cargo.toml adapters/fbneo/Cargo.toml
COPY adapters/retroarch/Cargo.toml adapters/retroarch/Cargo.toml
COPY services/relay/Cargo.toml services/relay/Cargo.toml

# Create stub sources so `cargo build` can resolve the workspace graph.
RUN mkdir -p apps/server/src packages/protocol/src packages/networking/src \
             packages/emulator-sdk/src packages/shared/src packages/game-definitions/src adapters/fbneo/src adapters/retroarch/src services/relay/src \
    && echo "fn main() {}" > apps/server/src/main.rs \
    && echo "" > packages/protocol/src/lib.rs \
    && echo "" > packages/networking/src/lib.rs \
    && echo "" > packages/emulator-sdk/src/lib.rs \
    && echo "" > packages/shared/src/lib.rs \
    && echo "" > packages/game-definitions/src/lib.rs \
    && echo "" > adapters/fbneo/src/lib.rs \
    && echo "" > adapters/retroarch/src/lib.rs \
    && echo "fn main() {}" > services/relay/src/main.rs \
    && cargo build --release --locked

# Copy the real sources (invalidates cache only when source changes).
COPY . .

# Remove only the dummy workspace artifacts before compiling the real sources. Dependency
# artifacts remain cached, while Cargo can no longer mistake a stub crate for the application.
RUN cargo clean --release -p opencade-server -p opencade-protocol -p opencade-shared \
    && cargo build --release --locked -p opencade-server \
    && test -x target/release/opencade-server

# --- runtime ---------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*

# Non-root user
RUN useradd -m -u 10001 opencade
WORKDIR /app

COPY --from=builder /app/target/release/opencade-server /usr/local/bin/opencade-server

# Migrations / static assets if present
COPY --from=builder /app/apps/server/migrations ./migrations

USER opencade
EXPOSE 8080
ENV RUST_LOG=info
ENV DATABASE_URL=postgres://opencade:opencade@db:5432/opencade
ENV OPENCADE_ENV=production

HEALTHCHECK --interval=10s --timeout=5s --retries=5 --start-period=15s \
  CMD wget -qO- http://localhost:8080/health || exit 1

CMD ["opencade-server"]
