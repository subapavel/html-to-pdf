# html-to-pdf — working agreement for humans and agents

HTML + Tailwind CSS → PDF **without a browser**. First product: Czech/EU invoices. Built on
[fulgur](https://github.com/fulgur-rs/fulgur) (Blitz/Stylo/Taffy/Parley + Krilla). Rust.

Read this file first. It is short on purpose; details live in `docs/`.

## What we are optimising for

1. **Precision** — output must match the CSS spec; Chrome is the *reference*, not the truth.
   When fulgur and Chrome disagree, cite the spec section before changing anything.
2. **Speed** — single-page invoice in low tens of ms, 200-page document under a second.
3. **Determinism** — same input → byte-identical PDF. Never read host fonts or the network.

## Layout of the repo

| Path | What |
|---|---|
| `crates/htmltopdf` | Library. Thin API over fulgur; the only place that imports `fulgur`. |
| `crates/htmltopdf-cli` | `htmltopdf render …` binary. |
| `fixtures/invoices/NN-name/` | One invoice per directory: `index.html`, `README.md` (what it exercises), `reference/` (Chrome PDF + PNGs, committed). |
| `fixtures/tailwind/` | Tailwind v4 source + **committed compiled** `tailwind.css`. Rebuild only when fixtures change. |
| `fixtures/fonts/` | Inter TTFs, fetched by `scripts/fetch-fonts.sh`, git-ignored. |
| `scripts/reference/` | Chromium reference renderer (Playwright). |
| `scripts/compare.py` | fulgur vs Chrome pixel diff → `out/compare/`. |
| `scripts/session-start.sh` | SessionStart hook: fonts + background release build. Cloud VM setup: `docs/cloud-environment.md`. |
| `docs/decisions/` | ADRs. One file per irreversible decision. Read before proposing architecture. |
| `docs/plans/` | Design doc per feature, written **before** implementation. |
| `docs/postmortems/` | What went wrong and why. Agents append here after a failed attempt. |
| `.claude/agents/` | Subagent roles with model/effort/tools. Expensive models never edit code. |

## Invariants (break one → the PR is wrong, however green CI is)

- `fulgur` is imported only in `crates/htmltopdf`. Bindings and CLI depend on `htmltopdf`.
- Layout coordinates are CSS px; PDF coordinates are PDF pt. Conversion happens in **one** place (fulgur's `units`). Never multiply by 72/96 anywhere in our code.
- Fonts are bundled bytes. `system_fonts(false)` stays. If a glyph is missing, bundle a font; do not enable host fonts.
- Fixtures are the spec. A behaviour change that moves pixels needs a fixture that shows it and an updated golden, in the same PR.
- `fixtures/tailwind/tailwind.css` is a build artefact but **committed**; the Rust side and cloud agents must never need npm.

## Who decides what

The owner is **not** a layout engineer and does not want to approve PRs or pick next steps.
Agents decide, act and record. The owner decides only:

- product scope (which documents and CSS features we support, in `docs/roadmap.md`),
- money and licences (paid services, any licence that is not MIT/Apache/OFL-style permissive),
- publishing (crates.io, npm, PyPI releases) and anything legal.

Everything else is yours. If you find yourself writing "tell me whether to…", stop: look for the
standing decision below, then in `docs/decisions/`; if it is truly new, decide, write an ADR, and
carry on. Ask the owner only when the decision is on the list above.

### Standing decisions (do not ask again)

| Situation | Decision |
|---|---|
| A fix belongs in fulgur / Blitz / Taffy / Parley | Fork under `subapavel/<crate>`, branch `htmltopdf/<slug>`, `[patch.crates-io]` in the workspace `Cargo.toml`, open the upstream PR, record both in `docs/upstream.md`. **Never vendor upstream source into this repo.** |
| The permission classifier denies an action | Do not work around it. Take the next option from the relevant standing decision; if none applies, write a postmortem and leave that step open. |
| Behaviour the user did not write would make the library refuse a document (unused CSS rules, `@media screen` blocks, framework leftovers) | Warn by default; strict mode is opt-in (`strict_fonts(true)`, `--strict-fonts`). Fixtures, tests and CI run strict. |
| A pixel-diff threshold blocks CI | The thresholds are a regression guard, not a target. Lower one in the same PR whenever the number improves. Raise one **only** in the case merge-policy item 3 already allows — growth from content that was missing and is now rendered — to the new number plus a 1-point margin, with a comment in `compare.py` naming the fixture, the cause and the finding that will bring it back down. Any other growth is a regression: fix the engine or the fixture. |
| What to work on next | The findings queue (`docs/plans/*-findings.md`, in the order given there), then `docs/roadmap.md`. One finding per PR. |
| Chrome references missing | They are committed under `fixtures/**/reference/`. If absent, regenerate with `scripts/reference/render.mjs` and commit them; do not silently work without them. |

### Merge policy

An agent merges its own PR (squash, delete branch) when **all** hold:

1. CI is green.
2. `vrt-validator` ran on the branch and reported no high-severity finding.
3. No fixture's pixel diff grew — except growth caused by content that was previously missing and is now rendered (text that was dropped, a border that was absent), which the PR must name with the fixture and the number. Page counts did not change except for fixtures listed in `PAGE_COUNT_MAY_DIFFER`.
4. The diff matches the plan's scope; nothing from another finding slipped in.
5. For anything touching the PDF writer path, fonts or metadata: `pdf-reviewer` ran and reported no defect.

If one fails, fix it on the branch. Leave a PR open only when the blocker is on the owner's list
above; then write exactly one question, two or three options, your recommendation, and the default
you will take if there is no answer within a day.

### Reporting to the owner

Eight lines or fewer, in the owner's language, no command output:
what changed *for the product* (e.g. "IBAN and VS are back in the PDF"), the side-by-side PNG that
shows it, the compare number before/after, what is next. Technical detail belongs in the PR and in
`docs/`, where the next agent will read it.

### Pacing: one item, then ask

Token budget is the owner's call, so a queue run goes one item at a time. After an item is merged
(or parked per the merge policy), send the report and end with exactly one line:
`Next: <id> — <one-line description>. Continue? (yes / stop)` — then wait. Anything other than a
clear yes means stop. Never start the next item unasked, and never ask mid-item.

## Workflow for a change

1. Issue → if non-trivial, `docs/plans/YYYY-MM-DD-<slug>.md` (problem, spec refs, acceptance fixtures, risks).
2. Fixture + failing test first. Then implement. Then `scripts/compare.py` and look at the PNGs.
3. PR: what changed, which fixture proves it, pixel-diff delta, links to spec/WPT/upstream issue.
4. If the attempt failed or was abandoned: 10 lines in `docs/postmortems/` so the next agent does not repeat it.

## Commands

```bash
scripts/fetch-fonts.sh                                   # once
cargo build --release -p htmltopdf-cli                   # ~10 min cold (Stylo), seconds warm
cargo test                                               # unit + fixture render tests
target/release/htmltopdf render fixtures/invoices/01-basic/index.html -o out/01.pdf \
  --css fixtures/tailwind/tailwind.css --font fixtures/fonts/Inter-Regular.ttf …   # see scripts/render-fixtures.sh
scripts/render-fixtures.sh                               # all fixtures → out/
python3 scripts/compare.py                               # fulgur vs Chrome → out/compare/*.png + summary
```

## Upstream

We depend on a pinned `fulgur` minor. Fixes that belong in fulgur or Blitz go **upstream as PRs**;
we carry a patch only while waiting. Track them in `docs/upstream.md`.
Known upstream gaps that hit invoices: `border-collapse` (Blitz #386), inline-level backgrounds, nested table fragmentation.

## Agents: which role for which job

| Job | Role (`.claude/agents/`) | Model |
|---|---|---|
| "What does the spec/WPT/Chrome say about X?" | `css-spec-researcher` | expensive, read-only + web |
| Write a plan doc with acceptance fixtures | `layout-planner` | expensive, read-only |
| Implement against red tests | `layout-engineer` | mid, can edit |
| Hunt regressions, run compare, break things | `vrt-validator` | mid, read + bash |
| Review a PR's PDF output (fonts, ToUnicode, PDF/A) | `pdf-reviewer` | expensive, read-only |
| Triage WPT failures, bumps, regenerate goldens | `triage` | cheap |

Rule of thumb: the most expensive model reads and decides; it never iterates on code.
