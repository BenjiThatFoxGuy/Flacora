# syntax=docker/dockerfile:1

# ── Stage 1: build Flacora from source ──────────────────────────────────────
FROM rust:1.86-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    libdbus-1-dev libudev-dev libwayland-dev libxkbcommon-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# 1) Copy only dependency manifests first so cargo fetch is cached when only
#    source files change. The git rev pins in Cargo.toml make this stable.
COPY Cargo.toml Cargo.lock ./
COPY server/Cargo.toml server/Cargo.toml
# Dummy src so cargo fetch can resolve the workspace without the real code.
RUN mkdir -p src server/src && echo 'fn main(){}' > src/main.rs && echo '' > src/lib.rs && echo 'fn main(){}' > server/src/main.rs
RUN cargo fetch

# 2) Patch the shader while deps are cached.
COPY docker/patch-shader.sh docker/patch-shader.sh
RUN sh docker/patch-shader.sh

# 3) Now copy the real source and build. Only this layer busts on code changes.
COPY . .
# Touch the crate roots so cargo doesn't skip them (dummy files are older).
RUN touch src/main.rs src/lib.rs server/src/main.rs
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

# Preserve the existing Compose port mapping. Native VNC uses 5900.
ENV WEB_LISTENING_PORT=8080 \
    DISPLAY_WIDTH=1280 \
    DISPLAY_HEIGHT=768 \
    KEEP_APP_RUNNING=1 \
    WEB_FILE_MANAGER=1 \
    WEB_FILE_MANAGER_ALLOWED_PATHS=/output
EXPOSE 8080
VOLUME ["/output"]
