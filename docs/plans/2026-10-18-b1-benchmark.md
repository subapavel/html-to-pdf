# B1 — A benchmark and a speed guard

Date: 2026-10-18. Status: **planned**. Not a finding: the owner asked whether we have a benchmark (we do not), and speed is goal
2 in `AGENTS.md` ("single-page invoice in low tens of ms, 200-page document under a second"). Since 2026-10-17 eight pagination
changes landed (F78, F30d, F83, F87, F83b, F30g, F30f); each `vrt-validator` timed a few documents by hand, nothing in the repo
measures speed or stops a regression.

## 1. Goal

- One command that measures render time of a fixed corpus and prints a table: `python3 scripts/bench.py`.
- A committed baseline (`bench/baseline.json`) and a check mode (`--check`) that fails on a regression, run in CI.
- The two `AGENTS.md` budgets as hard limits, so the goal is a test, not a sentence.
- Known slow shapes (F82 / F30d-d giant paragraph, F87f huge break-rule sheets, F83f deep avoid chains) measured and reported
  but not gated, so their findings have numbers and a fix shows up as a drop.

## 2. Design

- **Corpus** (`bench/`), generated deterministically by `scripts/bench.py --generate` (fixed seeds, committed output so the
  numbers do not move when the generator changes; Czech text from the fixtures, Inter fonts, compiled `tailwind.css`):
  - fixtures 01–16 (as rendered by `render-fixtures.sh`, same flags);
  - `b200-items`: a 200-page invoice, items table with repeated `thead` (the 02 shape × N);
  - `b200-text`: 200 pages of terms paragraphs (06/13 shapes, `break-inside-avoid` sections, F83b);
  - `b200-mixed`: 200 pages mixing tables, multicol terms (F30d), cards with `overflow-hidden` (F30f), an abspos stamp per
    page (F30g);
  - `slow-*` (report only): one 20 000-word `<p>`; a 1 MB sheet of layered break rules; 200 nested avoid levels.
- **Measurement**: the release CLI (`target/release/htmltopdf`, same flags as `render-fixtures.sh`), wall clock per run,
  1 warm-up + N runs (default 7, `--runs`), report min / median / max; also pages and output size. Single process per run
  (that is how customers call it); a `--threads` note is out of scope. Library-level timing (no process start) is a later
  step if process start turns out to dominate the 1-page numbers (record the CLI start-up cost of `htmltopdf --version` in the
  table so it is visible).
- **Baseline and check**:
  - `bench/baseline.json`: per document median ms and the machine label (`nproc`, CPU model from `/proc/cpuinfo`).
  - `--check`: fails when a gated document's median exceeds `baseline × 1.30 + 5 ms` **on the same machine label**; on a
    different machine only the hard budgets apply (CI runners differ from the dev VM). `--update-baseline` rewrites the file;
    lowering is always allowed, raising follows the same rule as `compare.py` thresholds (AGENTS.md standing decision: only
    for content that was missing and now renders, with a comment naming the cause).
  - Hard budgets (any machine): every fixture ≤ 100 ms median; each `b200-*` ≤ 1500 ms median on CI hardware (the 1 s goal is
    for the dev VM; record the CI number and tighten once seen). The CI numbers of the first run set the CI baseline file
    `bench/baseline-ci.json`, committed in a follow-up commit of this PR.
- **CI**: one step after `render-fixtures.sh`: `python3 scripts/bench.py --check --runs 5`, and upload the table as an
  artifact. If editing `.github/workflows/ci.yml` is denied by the permission classifier (it was once, for an unrelated
  change), do not work around it: keep the script, document the command in `AGENTS.md` Commands, and record the open step in
  §7 and the postmortem log.
- **Docs**: `AGENTS.md` Commands gains `python3 scripts/bench.py`; a short `docs/benchmarks.md` with the current table and
  how to read it; the findings with slow shapes (F82, F30d-d, F83f, F87f) get their bench document names.

## 3. Acceptance

- `python3 scripts/bench.py` prints the table for all documents on the dev VM; numbers recorded in §7 and `docs/benchmarks.md`.
- `--check` passes on main's binary; a deliberately slowed binary (e.g. `HTMLTOPDF_BIN` pointing at a wrapper that sleeps
  50 ms) makes it fail with a message naming the document — a test of the guard, run once and recorded.
- Deterministic corpus: `--generate` twice gives byte-identical files.
- No rendering change: fixtures 01–16 PDFs byte-identical; `cargo test` green.

## 4. Steps

1. Script + corpus generator + corpus; run, look at the table, sanity-check page counts (200 ± 5 for `b200-*`).
2. Baseline on the dev VM; `--check` and the slowed-binary test.
3. CI step (or the recorded denial), docs, findings cross-references.
4. PR; after the first CI run, commit `bench/baseline-ci.json` from the uploaded table and tighten the CI budget if far below.

## 5. Non-goals

Profiling and speeding anything up (separate findings); library-level microbenchmarks; memory measurement (a later step:
peak RSS via `/usr/bin/time -v` is cheap to add if wanted).

## 6. Result

(Filled in at landing.)
