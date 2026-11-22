#!/bin/sh
set -eu

export FLACORA_HOME=/config
export XDG_CURRENT_DESKTOP=GTK
export LIBGL_ALWAYS_SOFTWARE=1
# llvmpipe's Vulkan lacks SHADER_FLOAT16_IN_FLOAT32 (unpack2x16float),
# which Rune's uber shader needs. The GL backend works.
export WGPU_BACKEND=gl

# Preserve settings from previous installs, including any saved output path.
if [ ! -e /config/settings.json ]; then
    (set -C; printf '%s\n' '{"output":"/output"}' > /config/settings.json)
fi

# The native package picker is provided by the GTK desktop portal.
dbus-update-activation-environment DISPLAY XDG_CURRENT_DESKTOP XDG_RUNTIME_DIR
exec /opt/flacora/AppRun "$@"
