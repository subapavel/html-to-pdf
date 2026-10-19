# Benchmarks and the speed guard

`python3 scripts/bench.py` renders a fixed corpus with the release CLI and prints how long each document takes. `--check` turns it
into a guard (CI runs it). Plan: `docs/plans/2026-10-18-b1-benchmark.md`. AGENTS.md goal 2 is the reason: a single-page invoice in
low tens of ms, a 200-page document under a second.

```bash
cargo build --release -p htmltopdf-cli
python3 scripts/bench.py                      # table, all documents (about 55 s)
python3 scripts/bench.py b200-text 02-long    # only these
python3 scripts/bench.py --check --runs 5     # what CI runs (about 45 s): exit 1 on a regression
python3 scripts/bench.py --update-baseline    # rewrite bench/baseline.json from a full run
python3 scripts/bench.py --generate           # rewrite bench/corpus/ (byte-reproducible; commit the result)
HTMLTOPDF_BIN=/path/to/binary python3 scripts/bench.py   # measure another binary (same variable as render-fixtures.sh)
```

The table and the raw samples are also written to `out/bench/table.txt` and `out/bench/results.json` (CI uploads both as the `bench` artifact).

## What is measured

- Wall clock of one `htmltopdf render` process per run, with the flags of `scripts/render-fixtures.sh` (bundled Inter, compiled
  `tailwind.css`, `--strict-fonts --strict-images`). That is how customers call the CLI. Process start is 3 ms (`htmltopdf --version`,
  printed under the table), so it does not matter above the 10 ms documents.
- Gated documents: 1 untimed warm-up, then `--runs` (default 7) timed runs; min / median / max. Report-only (`slow-*`) documents run once.
- Pages come from `pdfinfo`, so the table shows when a change moves the page count.

## Corpus

Fixtures 01-16 are rendered from `fixtures/invoices/`. The rest is generated into `bench/corpus/` by `scripts/bench.py --generate`
(fixed seeds, Czech text from the fixtures, no clock) and **committed**, so the numbers do not move when the generator changes. A
corpus change is a baseline change: regenerate, `--update-baseline`, say so in the PR.

| Document | Shape | Gated |
|---|---|---|
| `b200-items` | One 4800-row items table, repeated `thead`, row `break-inside: avoid`, `@page` margin boxes with `counter(pages)` (the 02 shape x 80) | yes |
| `b200-text` | 1230 kept-together terms sections (`break-inside-avoid`, F83b) and every 25th a 37-clause avoid paragraph taller than a page (06 / 13 shapes) | yes |
| `b200-mixed` | 181 units of items table + two-column terms (F30d) + `overflow-hidden` card with a long paragraph (F30f) + abspos stamp (F30g) | yes |
| `slow-paragraph` | One `<p>` of 40 000 words (F82) | report only |
| `slow-multicol` | One `<p>` of 20 000 words in `columns:2` (F30d-d) | report only |
| `slow-breaks` | 24 000 layered break rules over 1000 elements (F87f) | report only |
| `slow-avoid` | 400 nested `break-inside-avoid` levels of 20 children (F83f) | report only |

The slow documents are sized to take a few seconds, not the minutes the findings quote (F82: 120 000 words), so a full run stays
under a minute. They are for watching a number go down when a finding is fixed.

## Current numbers

Release CLI of main `d532069`, dev VM (4x Intel Xeon @ 2.10GHz), median of 7 runs, 2026-10-18 (`bench/baseline.json`); the `b200-text` and `b200-mixed` rows were lowered by F99a
(parallel shaping, 2026-10-19, same VM):

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
| b200-text | 200 | 787 | 879 |
| b200-mixed | 201 | 811 | 862 |
| slow-paragraph (1 run) | 59 | 3777 | 3777 |
| slow-multicol (1 run) | 21 | 2460 | 2460 |
| slow-breaks (1 run) | 17 | 2809 | 2809 |
| slow-avoid (1 run) | 399 | 1247 | 1247 |

Reading it:

- A fixture is 17-57 ms, the single-page ones 18-29 ms: "low tens of ms" holds; 02-long (60 rows, 3 pages) is the slowest.
- **The 200-page goal is met by `b200-text` and `b200-mixed` since F99a** (0.88 s and 0.86 s; they were 1.12 s and 1.08 s) and not by `b200-items` (2.3-2.4 s), while 800 plain paragraphs make 160 pages in 0.35 s
  (F78's measurement). `b200-items` without its two margin boxes takes about 1.4 s, so the footers cost about 1.1 s (5 ms per page) and the table
  alone is over the goal. Filed as F99 in the findings; `docs/plans/2026-10-19-f99-speed-profile.md` says where the time goes, F99a (`2026-10-19-f99a-parallel-construct.md`) took the first step.
- The very first render of a large document can be much slower than the rest (`b200-items` took 5.7 s once, then 2.2-2.5 s; cause not investigated, probably cold caches); that is why gated documents get a warm-up.

## The guard (`--check`)

Applies to gated documents (fixtures and `b200-*`). A document fails when its median exceeds

- the **hard budget**, on any machine: 100 ms for a fixture, 1500 ms for a `b200-*`; `b200-items` is 6000 ms because main already needs
  2.4 s (`HARD_MS_OVERRIDE` in the script; lower it in the PR that makes that shape faster), or
- **baseline x 1.30 + 5 ms**, only when the machine label (`nproc` + CPU model) equals the one in `bench/baseline.json`. A CI runner has a
  different label, so there only the hard budgets apply until a `bench/baseline-ci.json` exists (plan §4 step 4: commit it from the first CI
  artifact).

A document over its limit is measured once more with twice the runs and the lower median counts, so one noisy sample does not fail a PR.
The failure message names the document, the median and the limit. Also a failing render (exit code != 0, or a timeout of a gated document) fails the check.

What it does not catch: a regression smaller than 30 % + 5 ms, and a fixed cost on a large document (+50 ms on a 2.4 s document is noise).
Tightening the factor needs a quieter machine than a shared VM. Tested once with a wrapper that sleeps 50 ms before `exec`ing the binary
(`HTMLTOPDF_BIN=wrapper.sh`): all 16 fixtures fail, naming each; with a 500 ms sleep `b200-text` fails (1664 ms against 1465 ms).

## Changing a number

- A change that makes a document faster: run `--update-baseline` in the same PR (lower is always allowed). Lower `HARD_MS_OVERRIDE` if it applies.
- A change that makes a document slower: a regression, fix it. The only exception is the `compare.py` rule: content that was missing and now
  renders (text that was dropped, a border that was absent). Then raise the baseline and say which document, by how much and why in the PR.
- A change that moves the page count of a `b200-*` outside 195-205 makes the table print a NOTE: the document no longer measures "200 pages".

## Findings measured here

F99 (200-page budget), F82 (`slow-paragraph`), F30d-d (`slow-multicol`), F83f (`slow-avoid`), F87f (`slow-breaks`). Each finding in
`docs/plans/2026-10-15-m0-findings.md` names its bench document.

## Not covered

Library-level timing (no process start), peak memory (F99a measured it once with `resource.getrusage` of the child: `/usr/bin/time` is not installed on the dev VM; 128-243 MB for the b200 documents), multi-threaded callers, and anything on CI hardware
until the first CI run has produced numbers.
