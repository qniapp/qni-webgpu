#!/usr/bin/env bash
#
# docs/* 配下の HTML / Markdown と AGENTS.md を一括 lint する。
#
# - textlint + prh: 用語ゆれ / ルー語の検出 (用語集は tools/docs-lint/prh.yml)
# - html-validate: HTML 構造の検証
# - markdownlint-cli2: Markdown スタイル
#
# Usage: scripts/lint-docs.sh
#
# 依存は tools/docs-lint/ に集約されている。初回は `pnpm install` が必要。

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
