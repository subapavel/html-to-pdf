@AGENTS.md

## Claude Code specifics

- Before touching layout or pagination code, read `docs/decisions/` and the relevant `docs/plans/`.
- Use the subagents in `.claude/agents/` by name for research, planning and review; do the implementation in the main session or via `layout-engineer`.
- Always look at the rendered PNGs (`out/compare/`) after a render change; do not trust `cargo test` alone for anything visual.
- Keep answers in the language the user writes in (Czech or English).
