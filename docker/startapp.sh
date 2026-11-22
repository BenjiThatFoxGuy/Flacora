#!/bin/sh
set -eu

export FLACORA_HOME=/config
export XDG_CURRENT_DESKTOP=GTK
export LIBGL_ALWAYS_SOFTWARE=1
# llvmpipe's Vulkan lacks SHADER_FLOAT16_IN_FLOAT32 (unpack2x16float),
# which Rune's uber shader needs. The GL backend works.
export WGPU_BACKEND=gl
# Log GPU pick so we can see which adapter and backend the app actually chose.
export RUNE_GPU_LOG=1

# Preserve settings from previous installs, including any saved output path.
if [ ! -e /config/settings.json ]; then
    (set -C; printf '%s\n' '{"output":"/output"}' > /config/settings.json)
fi

# The native package picker is provided by the GTK desktop portal.
dbus-update-activation-environment DISPLAY XDG_CURRENT_DESKTOP XDG_RUNTIME_DIR 2>/dev/null || true
exec /opt/flacora/AppRun "$@"
