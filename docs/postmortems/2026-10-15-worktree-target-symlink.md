# 2026-10-15 — lost the build directory by symlinking `target/` into a git worktree

**What was tried.** Reviewing PR #1 in a second checkout (`git worktree add`), with `ln -sfn <main>/target target`
inside the worktree to reuse the 7-minute Stylo build.

**What happened.** After `git worktree remove --force`, the main checkout's `target` was a dangling self-referencing
symlink and the compiled artefacts were gone. A full rebuild was needed.

**Rule.** Never symlink `target/` between checkouts. To share a build across worktrees or branches set
`CARGO_TARGET_DIR=/home/claude/html-to-pdf/target` (or `[build] target-dir` in `.cargo/config.toml`) — cargo
handles the sharing and the locking itself. For reviewing a PR, prefer `git checkout -b review FETCH_HEAD` in the
main checkout over a worktree; the branch's build reuses the cache automatically.
