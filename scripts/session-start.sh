#!/usr/bin/env bash
# SessionStart hook (see .claude/settings.json). Runs in every Claude Code session, local and cloud,
# with $CLAUDE_PROJECT_DIR = repo root. Must be fast and must not block: anything slow goes to the
# background and the agent's first `cargo build` simply waits on cargo's own lock.
#
# Cloud VM prerequisites (apt packages) are NOT installed here — they belong to the environment's
# setup script, see docs/cloud-environment.md.
set -u
cd "${CLAUDE_PROJECT_DIR:-$(dirname "$0")/..}"

# 1. Fonts (idempotent, ~2 MB)
[ -f fixtures/fonts/Inter-Regular.ttf ] || scripts/fetch-fonts.sh >/dev/null 2>&1 || echo "session-start: font fetch failed (offline?); run scripts/fetch-fonts.sh" >&2

# 2. Warm the release build in the background. Cold Stylo build is ~7 min on 2 cores.
if [ ! -x target/release/htmltopdf ]; then
  mkdir -p out
  nohup sh -c 'cargo build --release -p htmltopdf-cli > out/build.log 2>&1; echo "exit=$?" >> out/build.log' >/dev/null 2>&1 &
  echo "session-start: release build started in background → out/build.log (cargo commands will wait on the lock)"
fi
exit 0
