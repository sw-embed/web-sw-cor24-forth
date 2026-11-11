#!/usr/bin/env bash
set -euo pipefail

# Copy the Forth timeline (built in ../sw-forth-timeline, a fork of
# sw-apl's timeline) into pages/forth-timeline/, published at
# https://sw-embed.github.io/web-sw-cor24-forth/forth-timeline/.
# build-pages.sh leaves that directory alone.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
SRC="${FORTH_TIMELINE_DIR:-$PROJECT_DIR/../sw-forth-timeline}/pages"

[ -f "$SRC/index.html" ] || { echo "no timeline at $SRC" >&2; exit 1; }
rsync -a --delete --exclude='.gitignore' --exclude='*.d.ts' "$SRC/" "$PROJECT_DIR/pages/forth-timeline/"
echo "Copied $SRC -> pages/forth-timeline/"
