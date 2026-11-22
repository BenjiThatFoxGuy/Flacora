# Flacora in Docker

This image runs the fork's **headless server**: Flacora's download engine plus a
web UI, with no desktop, GPU or VNC involved. Paste links, manage the queue,
install extensions and browse the downloaded files from the browser.

## Run

```sh
mkdir -p config music
docker run -d --name flacora --restart unless-stopped \
  -p 127.0.0.1:8080:8080 \
  -v "$PWD/config:/config" \
  -v "$PWD/music:/output" \
  ghcr.io/benjithatfoxguy/flacora:latest
```

or with Compose:

```sh
docker compose up -d
```

Then open <http://localhost:8080>.

- `/config` holds settings, history, installed extensions and their data.
- `/output` holds downloads. New configurations default to `/output`.
- Both mounts are created if missing and chowned to the in-container `flacora`
  user (uid 1000) on startup.
- To share a library with Navidrome, mount the same host folder as Flacora's
  `/output` and leave Navidrome's music mount unchanged.
- Keep the port on a trusted network or behind an authenticated proxy; the web
  UI has no login.

## Extensions

Open **Extensions**, pick a `.fext` package and install it. Packages are listed
at <https://rentry.org/flacora>. FFmpeg is included in the image. Browser-backed
extensions download their browser on first use.

## Builds

GitHub Actions builds the image from source on every push to `main` and publishes
it to `ghcr.io/benjithatfoxguy/flacora` (`latest`, plus the short SHA). The image
is `linux/amd64`.
