# B1 — A benchmark and a speed guard

Date: 2026-10-18. Status: **done** (§6); the PR and the first CI run (step 4) are open. Not a finding: the owner asked whether we have a benchmark (we do not), and speed is goal
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
  §6 and the postmortem log.
- **Docs**: `AGENTS.md` Commands gains `python3 scripts/bench.py`; a short `docs/benchmarks.md` with the current table and
  how to read it; the findings with slow shapes (F82, F30d-d, F83f, F87f) get their bench document names.

## 3. Acceptance

- `python3 scripts/bench.py` prints the table for all documents on the dev VM; numbers recorded in §6 and `docs/benchmarks.md`.
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

Landed on branch `b1-benchmark` (tooling only: no Rust, no fork, no golden changed). Release CLI of main `d532069` (byte-identical to
`/tmp/claude-0/htmltopdf-main-d532069`), dev VM `4x Intel(R) Xeon(R) Processor @ 2.10GHz`, median of 7 runs (`bench/baseline.json`):

| Document | Pages | min ms | median ms |
|---|---:|---:|---:|
| 01-basic | 1 | 21.1 | 22.1 |
| 02-long | 3 | 53.8 | 57.0 |
| 03-logo-qr | 1 | 23.7 | 27.8 |
| 04-bilingual-vat | 1 | 23.1 | 25.7 |
| 05-fancy | 1 | 24.3 | 29.1 |
| 06-long-notes | 2 | 23.6 | 25.8 |
| 07-heading-margin | 2 | 21.8 | 25.0 |
| 08-terms-grid | 2 | 29.7 | 32.8 |
| 09-fixed-row-height | 3 | 23.3 | 25.6 |
| 10-stacked-items | 2 | 19.9 | 21.4 |
| 11-full-width-rows | 1 | 17.1 | 17.9 |
| 12-terms-columns | 2 | 25.6 | 27.9 |
| 13-terms-keep-together | 3 | 27.0 | 28.2 |
| 14-totals-keep-together | 2 | 19.3 | 21.2 |
| 15-paid-stamp | 2 | 20.4 | 22.0 |
| 16-terms-card | 2 | 22.7 | 25.1 |
| b200-items | 200 | 2357 | 2433 |
| b200-text | 200 | 1114 | 1123 |
| b200-mixed | 201 | 1032 | 1084 |
| slow-paragraph (1 run) | 59 | 3777 | 3777 |
| slow-multicol (1 run) | 21 | 2460 | 2460 |
| slow-breaks (1 run) | 17 | 2809 | 2809 |
| slow-avoid (1 run) | 399 | 1247 | 1247 |

CLI start-up (`htmltopdf --version`): 3 ms. Full run about 55 s; `--check --runs 5` about 42 s.

Acceptance (§3):

- Table: printed and recorded here and in `docs/benchmarks.md`.
- `--check --runs 5` on main's binary: exit 0.
- Guard test: `HTMLTOPDF_BIN` = wrapper that sleeps 50 ms then `exec`s the binary: exit 1, all 16 fixtures fail with their names
  (e.g. `01-basic: median 75.0 ms > 33.7 ms (baseline 22.1 ms x 1.30 + 5 ms)`); a 500 ms wrapper on `b200-text`: exit 1
  (`median 1664.4 ms > 1464.9 ms`). A 50 ms slowdown on the 200-page documents is below the 30 % margin and is not caught, by design.
- Deterministic corpus: `--generate` twice (also from another cwd and with another `PYTHONHASHSEED`): `diff -r` empty.
- No rendering change: `scripts/render-fixtures.sh` with the dev binary and with `/tmp/claude-0/htmltopdf-main-d532069`: the 16
  `htmltopdf.pdf` files are byte-identical (`cmp`). `cargo test --release --locked`: green.

Deviations from the plan:

1. **The 200-page goal is not met, so `b200-items` has its own hard budget.** All three `b200-*` take 1.08-2.43 s for 200 pages. The plan's
   1500 ms hard budget holds for `b200-text` and `b200-mixed` (1.1 s median, headroom 30 %: tight on a slower runner, see below), but
   `--check` could not pass on main for `b200-items` (one 4800-row table with `counter(pages)` margin boxes, 2.4 s; the same table without the
   margin boxes 1.4 s). `HARD_MS_OVERRIDE` sets 6000 ms for it and the cause is filed as **F99**. Page counts are 200 / 200 / 201.
2. `slow-multicol` (20 000 words in `columns:2`) is an extra report-only document, so that F30d-d has a bench name of its own.
3. Sizes of the report-only documents were tuned for a few seconds each: 40 000 words (`slow-paragraph`, 20 000 gave 1 s), 20 000 words in
   columns, 24 000 rules x 1000 elements, 400 levels x 20 children (400 x 100 was 4 s but 3.3 MB). The corpus is 6.4 MB of HTML.
4. A document over its limit is re-measured once with twice the runs and the lower median counts (noise guard, not in the plan).
5. The CI step was added to `.github/workflows/ci.yml` (the permission system allowed the edit): `python3 scripts/bench.py --check --runs 5`
   after the compare artifact upload, and a `bench` artifact with `out/bench/table.txt` and `results.json`. **Not run on CI**: the branch
   was not opened as a PR. Risk: on CI hardware `b200-text` / `b200-mixed` (1.1 s here, limit 1.5 s) may exceed the hard budget on the first
   run; then step 4 applies (read the artifact, commit `bench/baseline-ci.json`, set the limits from the numbers).
6. `bench/baseline-ci.json` (step 4) is not written: it needs the first CI run. The script does not read it yet; add that with the file.

Not verified: any CI hardware number; the speed guard's false-positive rate over days (one VM, a handful of runs). `cargo fmt --all -- --check`
and `cargo clippy --workspace --all-targets -- -D warnings` pass (no Rust file changed).
