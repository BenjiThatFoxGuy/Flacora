# syntax=docker/dockerfile:1

# ── Stage 1: build the headless server from source ──────────────────────────
FROM rust:1.86-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    libdbus-1-dev libudev-dev libwayland-dev libxkbcommon-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Manifests first: cargo fetch is cached until the dependency set changes.
COPY Cargo.toml Cargo.lock ./
COPY server/Cargo.toml server/Cargo.toml
RUN mkdir -p src server/src \
    && echo 'fn main(){}' > src/main.rs \
    && echo '' > src/lib.rs \
    && echo 'fn main(){}' > server/src/main.rs
RUN cargo fetch

# Real source: this layer (and the build) busts only when the code changes.
COPY . .
RUN touch src/main.rs src/lib.rs server/src/main.rs
RUN RUSTFLAGS="" cargo build --release -p flacora-server

# ── Stage 2: runtime ────────────────────────────────────────────────────────
# ffmpeg: extensions that remux or tag need it on PATH.
# The rest are the shared libraries Chromium links against: browser-backed
# extensions download their own Chromium into /config/cloakbrowser on first use.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl ffmpeg gosu \
    libasound2 libatk-bridge2.0-0 libatk1.0-0 libatspi2.0-0 \
    libcairo2 libcups2 libdbus-1-3 libdrm2 libgbm1 libglib2.0-0 \
    libnspr4 libnss3 libpango-1.0-0 libx11-6 libxcb1 libxcomposite1 \
    libxdamage1 libxext6 libxfixes3 libxkbcommon0 libxrandr2 libxtst6 \
    fonts-liberation \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -s /bin/sh flacora

COPY --from=builder /build/target/release/flacora-server /usr/local/bin/flacora-server
COPY --chmod=755 docker/entrypoint.sh /entrypoint.sh

ENV FLACORA_HOME=/config \
    FLACORA_OUTPUT=/output \
    FLACORA_HOST=0.0.0.0 \
    FLACORA_PORT=8080

EXPOSE 8080
VOLUME ["/config", "/output"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS --max-time 4 "http://127.0.0.1:${FLACORA_PORT}/api/version" || exit 1

ENTRYPOINT ["/entrypoint.sh"]
CMD ["flacora-server"]
