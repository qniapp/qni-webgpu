#!/usr/bin/env bash
set -euo pipefail

web_dir="$(dirname "$(dirname "$(realpath "$0")")")"
output="${1:-$web_dir/embed-dist}"
mkdir -p "$output"
output="$(realpath "$output")"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

env -u NO_COLOR -C "$web_dir" trunk build --release --dist "$stage"
# Fonts and shaders are compiled into wasm; the standalone bootstrap is not used.
cp "$stage/qni-web.js" "$stage/qni-web_bg.wasm" "$output/"
cp "$web_dir/embed.mjs" "$output/qni-embed.mjs"
printf 'Embed bundle: %s\n' "$output"
wc -c "$output/qni-embed.mjs" "$output/qni-web.js" "$output/qni-web_bg.wasm"
