#!/usr/bin/env bash
# Setup script for the Claude Code *cloud environment* (paste into the environment's
# "Setup script" field, or point it here: `bash scripts/cloud-setup.sh`).
# Runs once after the repo is cloned; the result is cached between sessions.
# Must be idempotent and must not need anything outside the default network allowlist
# (crates.io, github.com, apt mirrors, npm registry are allowed; arbitrary hosts are not).
set -euxo pipefail

# 1. System tools used by tests and the compare script
if command -v apt-get >/dev/null; then
  sudo apt-get update -qq
  sudo apt-get install -y -qq --no-install-recommends \
    build-essential pkg-config curl unzip \
    poppler-utils qpdf python3 python3-pil
fi

# 2. Rust (skip if present)
if ! command -v cargo >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal -c rustfmt,clippy
fi
source "$HOME/.cargo/env"

# 3. Project deps + a warm release build so the first agent turn does not pay for Stylo
cd "$(dirname "$0")/.."
scripts/fetch-fonts.sh
cargo fetch
cargo build --release -p htmltopdf-cli
cargo test --release --no-run

# 4. Sanity: render the fixtures once so out/ exists for compare.py
scripts/render-fixtures.sh
python3 scripts/compare.py || true   # thresholds may fail before M1; that is information, not an error here
