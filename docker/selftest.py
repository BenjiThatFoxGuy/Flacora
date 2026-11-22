#!/usr/bin/env python3
"""End-to-end check for the Flacora container.

Exercises the real pipeline against a synthetic extension package:
web UI serving, package install (unpack + manifest + exec bit), link parsing,
job spawn with the filled argument contract, the `@flacora` protocol, saved
file containment, and the staging -> /output placement.

Usage: selftest.py <image>
"""

import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import zipfile

HOST = "127.0.0.1"
SERVICE = "example.test"
EXT_ID = "synthetic-dl"
CATALOG_ID = "synthetic-catalog"

MANIFEST = {
    "id": EXT_ID,
    "name": "Synthetic Downloader",
    "version": "1.0.0",
    "kind": "downloader",
    "description": "CI fixture that writes one FLAC into {output}.",
    "services": ["Test"],
    "hosts": [SERVICE],
    "executable": {"linux-x64": "dl.sh"},
    "args": ["{url}", "--output", "{output}"],
}

CATALOG_MANIFEST = {
    "id": CATALOG_ID,
    "name": "Synthetic Catalog",
    "version": "1.0.0",
    "kind": "metadata",
    "description": "CI fixture that lists two tracks for any query.",
    "services": ["Test"],
    "hosts": [SERVICE],
    "catalog": True,
    "executable": {"linux-x64": "cat.sh"},
    "args": [],
}

# Mirrors what a real downloader prints so Flacora records the file as saved.
SCRIPT = """#!/bin/sh
# dl.sh <url> --output <dir>
out=""
while [ $# -gt 0 ]; do
    case "$1" in
        --output) out="$2"; shift 2 ;;
        *) shift ;;
    esac
done
if [ -z "$out" ]; then
    echo "no output directory given" >&2
    exit 3
fi
mkdir -p "$out"
target="$out/synthetic.flac"
printf 'fake flac\\n' > "$target"
echo "@flacora {\\"event\\":\\"stage\\",\\"text\\":\\"downloaded\\"}"
echo "@flacora {\\"event\\":\\"saved\\",\\"path\\":\\"$target\\"}"
"""

# A catalog provider answers --search/--browse with one catalog page.
CATALOG_SCRIPT = """#!/bin/sh
case "$1" in
    --search) title="Search: $2" ;;
    --browse) title="Album: $2" ;;
    *) title="Home" ;;
esac
echo "@flacora {\\"event\\":\\"catalog\\",\\"page\\":{\\"title\\":\\"$title\\",\\"note\\":\\"synthetic\\",\\"items\\":[\\"kind\\":\\"track\\",\\"title\\":\\"Track One\\",\\"detail\\":\\"Synthetic\\",\\"url\\":\\"https://example.test/track/1\\"},{\\"kind\\":\\"track\\",\\"title\\":\\"Track Two\\",\\"detail\\":\\"Synthetic\\",\\"url\\":\\"https://example.test/track/2\\"}]}}"
"""


def free_port():
    with socket.socket() as sock:
        sock.bind((HOST, 0))
        return sock.getsockname()[1]


def build_package(path, manifest, files):
    """A plain zip is accepted: unpack() falls back to zip when the FEXT magic is absent."""
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("extension.json", json.dumps(manifest, indent=2))
        for name, body in files.items():
            info = zipfile.ZipInfo(name)
            info.external_attr = 0o100755 << 16
            archive.writestr(info, body)


def get_json(url, timeout=5):
    with urllib.request.urlopen(url, timeout=timeout) as response:
        return response.status, json.loads(response.read().decode())


def post_json(url, payload):
    request = urllib.request.Request(
        url,
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=10) as response:
        return response.status


def wait_for(description, check, timeout=90):
    deadline = time.time() + timeout
    last = None
    while time.time() < deadline:
        try:
            last = check()
            if last:
                print(f"ok: {description}")
                return last
        except Exception as error:  # noqa: BLE001 - probe, keep polling
            last = error
        time.sleep(1)
    print(f"FAIL: {description} (last: {last})")
    return None


def run(image):
    root = tempfile.mkdtemp(prefix="flacora-selftest-")
    config = os.path.join(root, "config")
    output = os.path.join(root, "output")
    os.makedirs(config)
    os.makedirs(output)
    port = free_port()
    name = "flacora-selftest"
    base = f"http://{HOST}:{port}"

    subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
    subprocess.run(
        [
            "docker", "run", "-d", "--name", name,
            "-p", f"{HOST}:{port}:8080",
            "-v", f"{config}:/config",
            "-v", f"{output}:/output",
            image,
        ],
        check=True,
        capture_output=True,
    )

    try:
        state_url = f"{base}/api/state"

        loaded = wait_for(
            "engine loads",
            lambda: get_json(state_url)[1].get("loaded") is True and True,
        )
        if not loaded:
            return False

        for path, marker in (("/", "Flacora"), ("/app.js", "renderDownloads"), ("/style.css", "badge")):
            try:
                with urllib.request.urlopen(f"{base}{path}", timeout=5) as response:
                    status = response.status
                    body = response.read().decode()
            except urllib.error.HTTPError as error:
                status, body = error.code, ""
            if status != 200 or marker not in body:
                print(f"FAIL: {path} served (status {status})")
                return False
        print("ok: web UI assets served")

        package = os.path.join(root, "synthetic.fext")
        build_package(package, MANIFEST, {"dl.sh": SCRIPT})
        subprocess.run(
            ["curl", "-fsS", "-F", f"file=@{package}", f"{base}/api/extensions/install"],
            check=True,
            capture_output=True,
        )

        def extension_installed():
            state = get_json(state_url)[1]
            return any(item["id"] == EXT_ID for item in state["installed"]) and True

        if not wait_for("extension package installs", extension_installed):
            return False

        status = post_json(f"{base}/api/enqueue", {"urls": [f"https://{SERVICE}/track/1"]})
        if status not in (200, 204):
            print(f"FAIL: enqueue rejected ({status})")
            return False

        def job_finished():
            state = get_json(state_url)[1]
            if not state["jobs"]:
                return None
            job = state["jobs"][0]
            return job["status"] if job["status"] in ("done", "failed", "cancelled") else None

        finished = wait_for("queued job runs to completion", job_finished)
        if finished != "done":
            print(f"FAIL: job ended as {finished!r}")
            return False

        placed = [name_ for name_ in os.listdir(output) if os.path.isfile(os.path.join(output, name_))]
        if "synthetic.flac" not in placed:
            print(f"FAIL: download not placed in /output (found {placed})")
            return False
        print("ok: downloaded file placed in /output")

        # A second link must queue behind the first rather than being dropped.
        post_json(f"{base}/api/enqueue", {"urls": [f"https://{SERVICE}/track/2"]})
        queued = wait_for(
            "second link queues",
            lambda: len(get_json(state_url)[1]["jobs"]) >= 2 and True,
        )
        if not queued:
            return False

        # Catalog: a provider lists tracks, selecting them queues a download each.
        catalog_package = os.path.join(root, "synthetic-catalog.fext")
        build_package(catalog_package, CATALOG_MANIFEST, {"cat.sh": CATALOG_SCRIPT})
        subprocess.run(
            ["curl", "-fsS", "-F", f"file=@{catalog_package}", f"{base}/api/extensions/install"],
            check=True,
            capture_output=True,
        )

        def catalog_installed():
            state = get_json(state_url)[1]
            return any(item["id"] == CATALOG_ID for item in state["installed"]) and True

        if not wait_for("catalog extension installs", catalog_installed):
            return False

        before = len(get_json(state_url)[1]["jobs"])
        post_json(f"{base}/api/catalog/submit", {"input": "synthetic"})

        def catalog_listed():
            catalog = get_json(state_url)[1]["catalog"]
            return len(catalog["items"]) == 2 and True

        if not wait_for("catalog search lists tracks", catalog_listed):
            return False

        post_json(f"{base}/api/catalog/select-all", {"selected": True})
        post_json(f"{base}/api/catalog/queue", {})

        def catalog_queued():
            return len(get_json(state_url)[1]["jobs"]) >= before + 2 and True

        if not wait_for("catalog tracks queue for download", catalog_queued):
            return False

        print("PASS: end-to-end container check")
        return True
    finally:
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: selftest.py <image>")
    if not run(sys.argv[1]):
        sys.exit(1)
