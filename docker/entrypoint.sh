#!/bin/sh
# Run the headless server as the unprivileged `flacora` user, making sure the
# config and output mounts exist and are writable by it.
set -eu

mkdir -p /config /output

if [ "$(id -u)" = "0" ]; then
    chown -R flacora:flacora /config /output
    exec gosu flacora "$@"
fi

exec "$@"
