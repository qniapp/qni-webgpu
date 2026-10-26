#!/usr/bin/env bash
set -euo pipefail

web_dir="$(dirname "$(dirname "$(realpath "$0")")")"
output="${1:-$web_dir/embed-dist}"
mkdir -p "$output"
output="$(realpath "$output")"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

env -u NO_COLOR -C "$web_dir" trunk build --verbose --release --cargo-profile embed --dist "$stage"
cp "$stage/qni-web.js" "$stage/qni-web_bg.wasm" "$output/"
source_commit="${QNI_SOURCE_COMMIT:-${GITHUB_SHA:-$(git -C "$web_dir" rev-parse HEAD 2>/dev/null || true)}}"
if [[ ! "$source_commit" =~ ^[[:xdigit:]]{40}$ ]]; then
  source_commit=unknown
fi
sed "s/__QNI_SOURCE_COMMIT__/$source_commit/g" "$web_dir/embed.mjs" > "$output/qni-embed.mjs"
cp "$web_dir/qni-embed-error.mjs" "$output/"
printf 'Embed bundle: %s\n' "$output"
wc -c "$output/qni-embed-error.mjs" "$output/qni-embed.mjs" "$output/qni-web.js" "$output/qni-web_bg.wasm"
