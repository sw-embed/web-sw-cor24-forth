#!/usr/bin/env bash
set -euo pipefail

# Build pages/ for GitHub Pages deployment.
# Run this before committing pages/ changes.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Building pages/ ==="
cd "$PROJECT_DIR"
mkdir -p pages
touch pages/.nojekyll
trunk build --release --public-url /web-sw-cor24-forth/
# pages/forth-timeline/ is not Trunk's: scripts/update-forth-timeline.sh copies it in.
rsync -a --delete --exclude='.nojekyll' --exclude='forth-timeline/' dist/ pages/

echo "=== Done ==="
echo "Pages built in: $PROJECT_DIR/pages/"
echo "To deploy: git add pages/ && git commit && git push"
