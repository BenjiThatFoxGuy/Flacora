#!/bin/sh
# Patch rune's clip.wgsl to replace `unpack2x16float` (which requires
# SHADER_FLOAT16_IN_FLOAT32 - unsupported on llvmpipe) with a manual
# f16-to-f32 conversion that works on any GPU backend.
set -eu

CARGO_HOME="${CARGO_HOME:-/usr/local/cargo}"
CLIP=$(find "$CARGO_HOME" -path '*/rune*/crates/rune/src/*clip.wgsl' -print -quit 2>/dev/null || true)
if [ -z "$CLIP" ]; then
    echo "clip.wgsl not in CARGO_HOME, trying to force checkout..."
    cargo check --release 2>/dev/null || true
    CLIP=$(find "$CARGO_HOME" -path '*/rune*/crates/rune/src/*clip.wgsl' -print -quit 2>/dev/null || true)
fi
if [ -z "$CLIP" ]; then
    echo "ERROR: clip.wgsl not found anywhere under $CARGO_HOME" >&2
    find "$CARGO_HOME/git" -name 'clip.wgsl' 2>/dev/null || echo "(no clip.wgsl found)"
    exit 1
fi

echo "Patching $CLIP ..."

# Write the polyfill to a temp file and prepend it.
cat > /tmp/polyfill.wgsl << 'POLYFILL'
// --- llvmpipe polyfill: manual f16 -> f32 (no SHADER_FLOAT16_IN_FLOAT32) ---
fn _f16_bits_to_f32(h: u32) -> f32 {
    let sign = (h >> 15u) & 1u;
    let exp  = (h >> 10u) & 0x1Fu;
    let mant = h & 0x3FFu;
    if (exp == 0u) { return bitcast<f32>(sign << 31u); }
    if (exp == 31u) { return bitcast<f32>(sign << 31u | 0x7F800000u | mant << 13u); }
    return bitcast<f32>(sign << 31u | (exp + 112u) << 23u | mant << 13u);
}
fn soft_unpack2x16float(v: u32) -> vec2<f32> {
    return vec2<f32>(_f16_bits_to_f32(v & 0xFFFFu), _f16_bits_to_f32((v >> 16u) & 0xFFFFu));
}
// --- end polyfill ---
POLYFILL

cat /tmp/polyfill.wgsl "$CLIP" > /tmp/clip_patched.wgsl

# Replace the builtin call with our polyfill version.
sed -i 's/unpack2x16float(/soft_unpack2x16float(/g' /tmp/clip_patched.wgsl

cp /tmp/clip_patched.wgsl "$CLIP"

# Verify: the bare word should only appear as part of soft_unpack2x16float.
if grep -q '[^_]unpack2x16float\b' "$CLIP" 2>/dev/null; then
    echo "FAIL: bare unpack2x16float still present"
    grep 'unpack2x16float' "$CLIP"
    exit 1
fi

echo "OK - shader patched"
