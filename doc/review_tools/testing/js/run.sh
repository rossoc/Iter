#!/bin/sh
# Minimal JS runner: node:test for pure helpers, jsdom for DOM scripts. Run from anywhere.
# Usage: run.sh <dir-for-node_modules>   (jsdom is installed there, outside the repo)
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
DEPS=${1:-/tmp/iter-jsproof}
[ -d "$DEPS/node_modules/jsdom" ] || npm install --silent --no-audit --no-fund --prefix "$DEPS" jsdom@26.1.0
NODE_PATH="$DEPS/node_modules" node --test "$HERE"/*.test.mjs
