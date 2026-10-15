@AGENTS.md

## Claude Code specifics

- Before touching layout or pagination code, read `docs/decisions/` and the relevant `docs/plans/`.
- Use the subagents in `.claude/agents/` by name for research, planning and review; do the implementation in the main session or via `layout-engineer`.
- Always look at the rendered PNGs (`out/compare/`) after a render change; do not trust `cargo test` alone for anything visual.
- Reply in the language the user writes in (Czech or English). Everything committed to the repo —
  code comments, docs, plans, postmortems, commit messages, PR text — is always English.
  Fixture *content* (invoice text) is Czech on purpose; leave it.
