# Claude Code cloud environment

One environment named `html-to-pdf`, network access **Trusted** (crates.io, GitHub, apt, npm are in the
default allowlist). No secrets. Repo access comes from the GitHub app connection, not from a secret.

## Setup script (paste into the environment dialog)

Runs as root on Ubuntu 24.04 *before* Claude Code starts and *outside* the repo, so it may only
provision the VM. Must exit 0 and finish in under ~5 minutes or the environment is not cached.
Rust, Python, Node and git are pre-installed.

```bash
#!/bin/bash
apt-get update -qq && apt-get install -y -qq --no-install-recommends poppler-utils qpdf python3-pil unzip || true
```

## Everything else: SessionStart hook (in the repo)

`.claude/settings.json` runs `scripts/session-start.sh` on every session start, cloud or local:
fetches fonts and starts `cargo build --release` in the background. The agent's first cargo command
waits on cargo's lock, so nothing needs to be coordinated by hand. Build log: `out/build.log`.

## Starting a session

Model for the lead session: Sonnet is enough — planning and review run in the subagents from
`.claude/agents/`, whose models are fixed in their frontmatter.
