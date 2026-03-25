#!/usr/bin/env bash
set -euo pipefail

PORT=9181

exec trunk serve --port "$PORT" "$@"
