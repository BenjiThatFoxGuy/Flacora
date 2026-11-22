# syntax=docker/dockerfile:1

# Build stage: Ubuntu 22.04 so the binary matches the glibc the extensions ship for.
FROM ubuntu:22.04 AS build
ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        git \
        libdbus-1-dev \
        libudev-dev \
        libwayland-dev \
        libxkbcommon-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --no-modify-path --profile minimal --default-toolchain 1.99.1

WORKDIR /build
COPY . .
RUN cargo build --release -p flacora-server

# Runtime stage.
FROM ubuntu:22.04 AS runtime
ENV DEBIAN_FRONTEND=noninteractive \
    FLACORA_HOME=/config \
    FLACORA_OUTPUT=/output \
    FLACORA_HOST=0.0.0.0 \
    FLACORA_PORT=8080

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        ffmpeg \
        fonts-dejavu-core \
        fonts-liberation \
        libasound2 \
        libatk-bridge2.0-0 \
        libatk1.0-0 \
        libatspi2.0-0 \
        libcairo2 \
        libcups2 \
        libdbus-1-3 \
        libdrm2 \
        libgbm1 \
        libglib2.0-0 \
        libnspr4 \
        libnss3 \
        libpango-1.0-0 \
        libudev1 \
        libwayland-client0 \
        libx11-6 \
        libxcb1 \
        libxcomposite1 \
        libxdamage1 \
        libxext6 \
        libxfixes3 \
        libxkbcommon0 \
        libxrandr2 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /build/target/release/flacora-server /usr/local/bin/flacora-server

EXPOSE 8080
VOLUME ["/config", "/output"]

ENTRYPOINT ["/usr/local/bin/flacora-server"]
