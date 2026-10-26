#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LINT_DIR="$ROOT_DIR/tools/docs-lint"

if [ ! -d "$LINT_DIR/node_modules" ]; then
  echo "==> Installing docs-lint dependencies"
  (cd "$LINT_DIR" && pnpm install --frozen-lockfile=false)
fi

cd "$LINT_DIR"

status=0

echo "==> textlint (terminology / 用語ゆれ via prh)"
if ! pnpm run --silent lint:textlint; then
  status=1
fi

echo
echo "==> html-validate (HTML 構造)"
if ! pnpm run --silent lint:html; then
  status=1
fi

echo
echo "==> markdownlint (Markdown スタイル)"
if ! pnpm run --silent lint:md; then
  status=1
fi

exit "$status"
