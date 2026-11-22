# Flacora in Docker

This image runs the upstream Flacora desktop AppImage in the same kind of virtual desktop used by [docker-jdownloader-2](https://github.com/jlesage/docker-jdownloader-2). The browser displays the actual Flacora window. It does not use the fork's custom web frontend.

## Run

```sh
mkdir -p config music
docker run -d --name flacora --restart unless-stopped \
  -p 127.0.0.1:8080:8080 \
  -v "$PWD/config:/config" \
  -v "$PWD/music:/output" \
  ghcr.io/benjithatfoxguy/flacora:latest
```

Open <http://localhost:8080>. Native VNC is optional: publish container port `5900` if you need it. Keep the web desktop on a trusted network or behind an authenticated proxy; it is not authenticated by default. The browser port remains `8080`, so existing port mappings can stay as they are.

- `/config` holds Flacora settings, history, extensions and extension data. Existing settings are kept.
- `/output` holds downloads. New configurations default to `/output`; check **Settings → Download Folder** if an existing configuration points elsewhere.
- `USER_ID` and `GROUP_ID` default to `1000`. Set them to match the owner of your mounts if needed. Flacora needs write access to both paths.
- To use the same music folder as Navidrome, mount that host folder as Flacora's `/output` and leave Navidrome's existing music mount unchanged.

## Extensions

To install a trusted `.fext`, upload it to `/output` using the browser desktop's file manager. Then open **Extensions** inside Flacora and select the package from `/output`. FFmpeg comes with the container. Browser-backed extensions may need their own first-run setup.

## Builds

GitHub Actions downloads the pinned `linux-x64` AppImage from [upstream Releases](https://github.com/spotbye/Flacora/releases), checks its SHA-256, and extracts it at build time. No FUSE device, GPU passthrough, or privileged mode is required. The image is `linux/amd64`.
