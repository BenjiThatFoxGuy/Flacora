# syntax=docker/dockerfile:1
FROM ubuntu:24.04 AS upstream
ARG FLACORA_VERSION=1.0.2
ARG FLACORA_SHA256=1765a3d71c5e5a44e09de5ffdd07b5dc62fc2bc4d96829aaa20dbf05c1e294ec
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl
WORKDIR /tmp/flacora
RUN curl --fail --location --retry 3 -o Flacora.AppImage \
      "https://github.com/spotbye/Flacora/releases/download/v${FLACORA_VERSION}/Flacora-${FLACORA_VERSION}-linux-x64.AppImage" \
    && printf '%s  Flacora.AppImage\n' "$FLACORA_SHA256" | sha256sum -c - \
    && chmod +x Flacora.AppImage \
    && ./Flacora.AppImage --appimage-extract >/dev/null \
    && test -x squashfs-root/AppRun

# Same Xvnc + browser desktop base as docker-jdownloader-2, with glibc for the AppImage.
FROM jlesage/baseimage-gui:ubuntu-24.04-v4.14.0
RUN add-pkg ca-certificates dbus-bin ffmpeg fonts-dejavu-core fonts-liberation \
    libasound2t64 libatk-bridge2.0-0t64 libcups2t64 libgbm1 libgl1 libgl1-mesa-dri \
    libegl1 libnspr4 libnss3 libvulkan1 mesa-vulkan-drivers libudev1 \
    libwayland-client0 libx11-xcb1 libxcb-xfixes0 libxcomposite1 libxdamage1 \
    libxrandr2 libxkbcommon0 libxkbcommon-x11-0 xdg-desktop-portal-gtk \
    && set-cont-env APP_NAME Flacora

COPY --from=upstream /tmp/flacora/squashfs-root/ /opt/flacora/
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
