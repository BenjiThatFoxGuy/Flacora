# syntax=docker/dockerfile:1

# ── Stage 1: build Flacora from source ──────────────────────────────────────
FROM rust:1.86-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    libdbus-1-dev libudev-dev libwayland-dev libxkbcommon-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy everything, fetch, patch, build - no layer caching tricks.
COPY . .
RUN cargo fetch
RUN sh docker/patch-shader.sh

# Nuke any cached rune artifacts so the patched shader is recompiled.
RUN find target -path '*/rune-*' -name '*.d' -delete 2>/dev/null; \
    find target -path '*/rune-*' -name '*.rlib' -delete 2>/dev/null; \
    find target -path '*/rune-*' -name '*.rmeta' -delete 2>/dev/null; \
    rm -rf target/release/.fingerprint/rune-* target/release/build/rune-* || true

RUN RUSTFLAGS="" cargo build --release --bin flacora

# ── Stage 2: runtime on jlesage/baseimage-gui ───────────────────────────────
FROM jlesage/baseimage-gui:ubuntu-24.04-v4.14.0

RUN add-pkg ca-certificates dbus-bin ffmpeg fonts-dejavu-core fonts-liberation \
    libasound2t64 libatk-bridge2.0-0t64 libcups2t64 libgbm1 libgl1 libgl1-mesa-dri \
    libegl1 libnspr4 libnss3 libvulkan1 mesa-vulkan-drivers libudev1 \
    libwayland-client0 libx11-xcb1 libxcb-xfixes0 libxcomposite1 libxdamage1 \
    libxrandr2 libxkbcommon0 libxkbcommon-x11-0 xdg-desktop-portal-gtk \
    && set-cont-env APP_NAME Flacora \
    && set-cont-env WGPU_BACKEND gl \
    && set-cont-env LIBGL_ALWAYS_SOFTWARE 1

COPY --from=builder /build/target/release/flacora /opt/flacora/flacora
COPY --chmod=755 docker/startapp.sh /startapp.sh

ENV WEB_LISTENING_PORT=8080 \
    DISPLAY_WIDTH=1280 \
    DISPLAY_HEIGHT=768 \
    KEEP_APP_RUNNING=1 \
    WEB_FILE_MANAGER=1 \
    WEB_FILE_MANAGER_ALLOWED_PATHS=/output
EXPOSE 8080
VOLUME ["/output"]
