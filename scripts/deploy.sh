#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

if ! git diff --quiet HEAD -- pages/ 2>/dev/null; then
    echo "pages/ has uncommitted changes. Aborting."
    exit 1
fi

WASM_SRC="dist/web-sw-cor24-forth*.js"
WASM_DST="pages/web-sw-cor24-forth*.js"

src_hash=""
dst_hash=""

if ls $WASM_SRC 1>/dev/null 2>&1; then
    src_hash=$(sha256sum $WASM_SRC 2>/dev/null | cut -d' ' -f1 || true)
fi
if ls $WASM_DST 1>/dev/null 2>&1; then
    dst_hash=$(sha256sum $WASM_DST 2>/dev/null | cut -d' ' -f1 || true)
fi

if [[ "$src_hash" == "$dst_hash" && -n "$src_hash" ]]; then
    echo "pages/ is up to date. Nothing to deploy."
    exit 0
fi

echo "pages/ is stale. Rebuilding..."
./scripts/build-pages.sh

git add pages/
if git diff --cached --quiet pages/; then
    echo "pages/ unchanged after rebuild."
    exit 0
fi

git commit -m "Deploy: rebuild pages/ for GitHub Pages"
git push
echo "=== Deployed ==="
