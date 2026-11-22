#!/bin/sh
# Patch rune's clip.wgsl to replace `unpack2x16float` (which requires
# SHADER_FLOAT16_IN_FLOAT32 — unsupported on llvmpipe) with a manual
# f16-to-f32 conversion that works on any GPU backend.
set -eu

# cargo fetch stores bare repos in git/db; checkouts appear only during build.
# Search both locations, plus the entire CARGO_HOME as a fallback.
CARGO_HOME="${CARGO_HOME:-/usr/local/cargo}"
CLIP=$(find "$CARGO_HOME" -path '*/rune/crates/rune/src/shaders/clip.wgsl' -print -quit 2>/dev/null || true)
if [ -z "$CLIP" ]; then
    echo "clip.wgsl not in CARGO_HOME, trying to force checkout..."
    # A quick cargo check with an empty main forces git dep checkout.
    cargo check --release 2>/dev/null || true
    CLIP=$(find "$CARGO_HOME" -path '*/rune/crates/rune/src/shaders/clip.wgsl' -print -quit 2>/dev/null || true)
fi
if [ -z "$CLIP" ]; then
    echo "ERROR: clip.wgsl not found anywhere under $CARGO_HOME" >&2
    find "$CARGO_HOME/git" -name 'clip.wgsl' 2>/dev/null || echo "(no clip.wgsl found)"
    exit 1
fi

echo "Patching $CLIP ..."

# Prepend the polyfill functions and replace the builtin call.
sed -i '1i\
// --- llvmpipe polyfill: manual f16 -> f32 (no SHADER_FLOAT16_IN_FLOAT32) ---\
fn _f16_bits_to_f32(h: u32) -> f32 {\
    let sign = (h >> 15u) & 1u;\
    let exp  = (h >> 10u) & 0x1Fu;\
    let mant = h & 0x3FFu;\
    if (exp == 0u) { return bitcast<f32>(sign << 31u); }\
    if (exp == 31u) { return bitcast<f32>(sign << 31u | 0x7F800000u | mant << 13u); }\
    return bitcast<f32>(sign << 31u | (exp + 112u) << 23u | mant << 13u);\
}\
fn soft_unpack2x16float(v: u32) -> vec2<f32> {\
    return vec2<f32>(_f16_bits_to_f32(v & 0xFFFFu), _f16_bits_to_f32((v >> 16u) & 0xFFFFu));\
}\
// --- end polyfill ---
' "$CLIP"

sed -i 's/unpack2x16float(/soft_unpack2x16float(/g' "$CLIP"

echo "Patched. Verifying no unpack2x16float remains:"
grep -c 'unpack2x16float' "$CLIP" && { echo "FAIL: builtin call still present"; exit 1; } || echo "OK"
