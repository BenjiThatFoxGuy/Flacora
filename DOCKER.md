# Flacora web UI (Docker)

A headless Flacora: the same download engine as the desktop app, served with a
browser UI. Runs in Docker on a server with no display.

- Downloads queue: paste links, pick a downloader, watch progress and logs.
- Library: every saved track, with lyrics/metadata lookups.
- Extensions: install `.fext` packages from the web UI.
- Settings: output folder, naming, folders, auto-tagging, preferred downloaders.
- Files: browse and download the output folder.

## Run

```sh
mkdir -p config music
docker run -d --name flacora --restart unless-stopped \
  -p 8080:8080 \
  -v "$PWD/config:/config" \
  -v "$PWD/music:/output" \
  ghcr.io/benjithatfoxguy/flacora:latest
```

Then open <http://localhost:8080>.

`compose.yaml` in this repository does the same thing:

```sh
docker compose up -d
```

## Volumes

| Path       | What goes there                                          |
| ---------- | -------------------------------------------------------- |
| `/config`  | Settings, history, installed extensions, extension data, CloakBrowser, cached covers. |
| `/output`  | Downloaded music (the default output folder).            |

## Environment

| Variable          | Default     | Meaning                       |
| ----------------- | ----------- | ----------------------------- |
| `FLACORA_HOST`    | `0.0.0.0`   | Bind address.                 |
| `FLACORA_PORT`    | `8080`      | HTTP port.                    |
| `FLACORA_HOME`    | `/config`   | Settings/extensions root.     |
| `FLACORA_OUTPUT`  | `/output`   | Default download folder.      |

## Extensions

Download `.fext` packages from <https://rentry.org/flacora> and upload them in
the Extensions tab. Browser-backed downloaders download CloakBrowser into
`/config/cloakbrowser` the first time they need it, so keep `/config` on a
volume with a few GB free.

FFmpeg is installed in the image; extensions that need it find it on `PATH`.

## Build

```sh
docker build -t flacora .
```

The image is built and published to GHCR on every push to `main`
(`.github/workflows/build.yml`).
