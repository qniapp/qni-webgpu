#!/bin/bash
set -e

MAX_ITERATIONS="${1:-10}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "========================================"
echo " Ralph - Autonomous AI Coding Loop"
echo "========================================"
echo "Max iterations: $MAX_ITERATIONS"
echo ""

cd "$REPO_ROOT"

for i in $(seq 1 $MAX_ITERATIONS); do
  echo ""
  echo "════════════════════════════════════════"
  echo " Iteration $i of $MAX_ITERATIONS"
  echo "════════════════════════════════════════"
  echo ""

  OUTPUT=$(cat "$SCRIPT_DIR/prompt.md" \
    | claude --dangerously-skip-permissions 2>&1 \
    | tee /dev/stderr) || true

  if echo "$OUTPUT" | grep -q "<promise>COMPLETE</promise>"; then
    echo ""
    echo "✅ All stories completed!"
    exit 0
  fi

  echo ""
  echo "--- Iteration $i finished, waiting 2s ---"
  sleep 2
done

echo ""
echo "⚠️ Max iterations ($MAX_ITERATIONS) reached"
exit 1
