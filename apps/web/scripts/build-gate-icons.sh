#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_DIR="${ROOT_DIR}/src/assets/gates"
OUT_DIR="${SRC_DIR}/png"
SIZE=96

if ! command -v rsvg-convert >/dev/null 2>&1; then
  echo "rsvg-convert not found; skipping gate icon generation." >&2
  exit 0
fi

mkdir -p "${OUT_DIR}"

for svg in "${SRC_DIR}"/*.svg; do
  base="$(basename "${svg}" .svg)"
  rsvg-convert -w "${SIZE}" -h "${SIZE}" "${svg}" -o "${OUT_DIR}/${base}.png"
done

echo "Gate icon PNGs updated in ${OUT_DIR}"
