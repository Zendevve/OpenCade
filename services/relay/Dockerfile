# syntax=docker/dockerfile:1

# --- builder ---------------------------------------------------------------
FROM rust:1.98-bookworm AS builder
WORKDIR /app

# Copy workspace manifests for caching
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

# Create stub sources so workspace resolves
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
    && cargo build --release --locked -p opencade-relay

# Copy real sources
COPY . .

RUN cargo clean --release -p opencade-relay -p opencade-protocol -p opencade-shared \
    && cargo build --release --locked -p opencade-relay \
    && test -x target/release/opencade-relay

# --- runtime ---------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 10001 opencade
WORKDIR /app

COPY --from=builder /app/target/release/opencade-relay /usr/local/bin/opencade-relay

USER opencade
EXPOSE 8081

ENV RUST_LOG=info
ENV PORT=8081

HEALTHCHECK --interval=10s --timeout=5s --retries=5 --start-period=10s \
  CMD wget -qO- http://localhost:8081/health || exit 1

CMD ["opencade-relay"]
