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
python3 scripts/bench.py --scaling            # one long table at 1200 / 2400 / 4800 rows, five variants, and r = t(4800) / t(1200) (about 40 s)
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

## Scaling of one long table (F99c)

`--scaling` renders the `b200-items` table with 1200, 2400 and 4800 rows (generated into `out/bench/scaling/`, not committed;
`gen_items(rows, boxes, collapse, avoid, thead)` makes the corpus document with its defaults) and prints the median and
r = t(4800) / t(1200): about 4 when the work is linear in the rows, tending to 16 when it is quadratic. Variants say where a
superlinear cost is: `t` (the corpus table with a plain `@page`), `t-sep` (`border-collapse: separate`), `t-noavoid` (no row
`break-inside: avoid`), `t-nohead` (no repeated header), `t-boxes` (the corpus shape, with the margin boxes), `t-names` (F144: the table on a named page, `page: items`, and a forced break before the totals; ungated).

`--check` gates variant `t`: r must not exceed `SCALING_MAX_RATIO` (6.0, a guard against quadratic work only — the branch itself spreads up to 4.6 on a loaded VM, so a tighter gate would flake; re-measured once with twice the runs, the lower r
counts). It is a ratio of two timings of the same binary, so it holds on any machine; a CI runner is as good as the dev VM.
Resolution: r moves by about 0.25 between runs on the dev VM, so the gate catches a table that is clearly superlinear (main before F99c:
4.5-5.0, 5.0 on one thread; F99c: 4.0-4.4) and only partly a single per-page scan creeping back (it adds 0.4-0.6).
Measured 2026-10-20, 4 vCPU Xeon @ 2.80GHz, median of 5, main `fbaf57f` -> F99c (`b200-items` 4800 rows: 1.6-2.3 s, so the table is most of it):

| variant | main r | F99c r |
|---|---:|---:|
| t | 4.62-4.68 | 4.07-4.40 |
| t-sep | 4.19-4.26 | 4.00-4.21 |
| t-noavoid | 4.69-4.98 | 4.37-4.45 |
| t-nohead | 4.16-5.03 | 4.22-4.30 |
| t-boxes | 4.44-5.69 | 4.13-4.44 |

F99d (2026-10-20, Taffy fork `7bbedd5`: the two per-axis booleans of `GridItem::minimum_contribution` are computed once per axis instead of once per item) took the Taffy part out of that
cost; `docs/plans/2026-10-20-f99d-taffy-minimum-contribution.md` §7. Same host, binaries alternating, `--scaling` with 5 runs:

| variant | r before (F99c) | r after (F99d) |
|---|---:|---:|
| t | 4.07-4.40 | 3.90-4.29 |
| t-sep | 4.00-4.21 | 4.28 |
| t-noavoid | 4.37-4.45 | 4.29 |
| t-nohead | 4.22-4.30 | 3.85 |
| t-boxes | 4.13-4.44 | 4.08 |

The ratio moves little because the Taffy cost sat inside the superlinear part only partly (the profile of 1 thread, `t`: `GridItem::minimum_contribution` self time 19 / 53 / 233 / 1107 ms at 1200 / 2400 / 4800 / 9600 rows before,
1 / 0 / 1 / 3 after; `resolve_intrinsic_track_sizes` inclusive 44 / 113 / 369 / 1453 -> 29 / 60 / 112 / 346, r(4800/1200) 8.4 -> 3.9); the absolute time is what changed (`t` at 4800 rows under perf 2042 -> 1665 ms, at 9600 rows 5012 -> 4037 ms).
What is left of the table is spread over parse, style, shaping, convert and paint, with the pagination scans of F144 the next known superlinear one.

F144 (2026-10-20, fulgur fork `7ae03d0`: the fragmenter's "forced break below" and "page-name change below" questions are answered from a table built once per pass) took those scans out. Profile, 1 thread, `perf` on a symbolised build, inclusive samples at 1200 / 2400 / 4800 rows
(`has_forced_break_below` + `has_page_name_change_below` + `walk_used_page_names` + `SubtreeFacts::build`):

| variant | main | F144 |
|---|---:|---:|
| `t` | 12 / 27 / 52 | 0 / 0 / 0 |
| `t-names` (new: the table has `page: items`, the totals `break-before: page`) | 11 / 34 / 58 | 5 / 14 / 21 |

`t-names` still pays for `walk_used_page_names` (5 / 14 / 18, linear: F148) because the document names a page; `SubtreeFacts::build` is 0 / 0 / 3. `--scaling` now prints `t-names` too (ungated). The scans were O(n x levels), not quadratic, so r of `t` does not move; the gain is the work removed: wall clock, 1 thread, 15 interleaved runs, median:
`t` at 4800 rows 1563 -> 1485 ms (-78 ms, -5.0 %), `t-names` 1588 -> 1523 ms (-65 ms, -4.1 %). With 4 threads the difference is inside the VM's noise (`b200-items`: +24 ms in 15 runs, -44 ms in 25; CPU time unchanged).

## Current numbers

Release CLI of main `d532069`, dev VM (4x Intel Xeon @ 2.10GHz), median of 7 runs, 2026-10-18 (`bench/baseline.json`); the `b200-text` and `b200-mixed` rows were lowered by F99a
(parallel shaping, 2026-10-19, same VM), the `b200-items` and `02-long` rows by F99b (margin-box documents share one style and font environment, 2026-10-19, same VM):

| Document | Pages | min ms | median ms |
|---|---:|---:|---:|
| 01-basic | 1 | 21.1 | 22.1 |
| 02-long | 3 | 37.9 | 41.6 |
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
| b200-items | 200 | 1443 | 1575 |
| b200-text | 200 | 787 | 879 |
| b200-mixed | 201 | 811 | 862 |
| slow-paragraph (1 run) | 59 | 3777 | 3777 |
| slow-multicol (1 run) | 21 | 2460 | 2460 |
| slow-breaks (1 run) | 17 | 2809 | 2809 |
| slow-avoid (1 run) | 399 | 1247 | 1247 |

Reading it:

- A fixture is 17-42 ms, the single-page ones 18-29 ms: "low tens of ms" holds; 02-long (60 rows, 3 pages) is the slowest (it was 57 ms before F99b: its footer is a page-number box).
- **The 200-page goal is met by `b200-text` and `b200-mixed` since F99a** (0.88 s and 0.86 s; they were 1.12 s and 1.08 s) and not by `b200-items` (1.6 s since F99b, 2.3-2.4 s before), while 800 plain paragraphs make 160 pages in 0.35 s
  (F78's measurement). `b200-items` without its two margin boxes takes about 1.4 s, so the table alone is over the goal and the footers, which cost about 1.1 s (5 ms per page) before F99b, are
  down to roughly 0.2 s (inferred from 1.57 s minus 1.4 s, not re-measured without the footers). Filed as F99 in the findings; `docs/plans/2026-10-19-f99-speed-profile.md` says where the time goes,
  F99a (`2026-10-19-f99a-parallel-construct.md`) and F99b (`2026-10-19-f99b-margin-documents.md`) took the first two steps.
- **F99e (2026-10-26, the 2.10 GHz dev VM, so `bench/baseline.json` was rewritten from this run: every gated row went
  down; `slow-breaks`, report-only, keeps its old value).** Krilla deflates page streams on rayon's pool (`parallel-pdf`),
  each font face is parsed once per render, the bundled font bytes are shared instead of copied per conversion; output
  byte-identical. `--update-baseline` medians: `b200-items` 1575 -> 1190 ms, `b200-text` 879 -> 787 ms, `b200-mixed`
  862 -> 785 ms, `02-long` 41.6 -> 29.4 ms, the single-page fixtures 15-25 ms (part of the drop on the fixtures and
  `slow-*` rows is earlier work since the 2026-10-18 baseline). Interleaved A/B against main, 9-15 runs: `b200-items`
  1355-1360 -> 1224-1227 ms, `b200-text` 897-911 -> 773-788 ms, `b200-mixed` 878-902 -> 788-802 ms, `01-basic` 19.1 ->
  17.0 ms. The pool is now started for every render: about +1 ms of CPU at 16 threads and +9 ms at 64 on a one-page
  document (F179). `b200-items` is still over the goal; next: the margin-box documents in parallel (F99f).
- **F99c (2026-10-20, a different host: 4 vCPU Xeon @ 2.80GHz, so `bench/baseline.json`, recorded on the 2.10GHz host, was not rewritten: `--check` here applies only
  the hard budgets).** Main `fbaf57f` against the F99c branch, alternating, median of 7-11: `b200-items` 2312-2333 -> 1953-1970 ms (-342 / -380 ms, -15 / -16 %),
  `b200-text` 1074 / 947 -> 1024 / 990 and `b200-mixed` 1075 / 1127 -> 1147 / 1110 ms (no long table in them: within the +-5 % noise of the VM), `02-long` 55.2 -> 55.8 ms.
  The numbers of this host are about 1.4 times those of the baseline's host for the same binary (main's `b200-items` was 1575 ms there), so the 1 s goal
  cannot be judged here; scaled by that ratio the branch would take about 1.3 s on the baseline host (an estimate, not measured), over the goal. The change removes
  about a seventh of `b200-items`; the rest of the table is mainly Taffy (F99d).
- **F99d (2026-10-20, same 2.80GHz host, `bench/baseline.json` and `HARD_MS_OVERRIDE` again not touched: the machine label differs).** Main `6ad12cb` (F99c) against the Taffy fork, alternating, median of 7: `b200-items` 1884 / 1929 -> 1673 / 1619 ms (-211 / -310, -11 / -16 %);
  15 interleaved runs of each binary: 1829 -> 1601 ms wall (-228 ms, -12.5 %), CPU 2080 -> 1860 ms. `b200-mixed` 1223 / 1160 -> 1112 / 1036, `02-long` 51.9 / 48.2 -> 48.0 / 47.3. `b200-text` shows +4 / +6 % in the first two A B pairs and +10 / +3 % in two more (11 runs), but 975 -> 973 ms in the 15-run interleaved comparison:
  it has no table, so this is the VM's noise, not the change. The 1 s goal is not met for `b200-items` here (1.6-1.7 s; scaled by the host ratio of about 1.4 it would be about 1.2 s on the baseline host, an estimate, not measured).
- **F144 (2026-10-20, same 2.80GHz host, baseline and `HARD_MS_OVERRIDE` not touched).** Main `7397e3a` against the fork, 15 interleaved runs of each binary, 4 threads, median wall (CPU) in ms: `b200-items` 1637 -> 1661 (1906 -> 1902), and 1563 -> 1519 in a second series of 25 runs; `slow-breaks` 3309 -> 3315 (3346 -> 3342); `t-names` at 4800 rows 1331 -> 1365 (1569 -> 1611); `b200-text` 914 -> 904; `b200-mixed` 995 -> 1025. All inside the +-3 % the VM spreads, so the plan's stated target (`b200-items` -20 ms) is not demonstrated with 4 threads; the work removed shows with 1 thread (see the scaling section, `t` -78 ms) and in the profile (52 samples -> 0). `slow-breaks` does not use the table: `break-before: avoid` is not a forced break.
- **F99f step 2 (2026-10-27, the 2.10 GHz dev VM; `bench/baseline.json` and `HARD_MS_OVERRIDE` not touched: the effect is below the VM's noise, and this run's medians are above the baseline, so nothing may be lowered).** (a) The HTML is walked once for the font check, the image check and the four rewrites (was six walks with a `String` per tag): in-process on `b200-items.html` (1.9 MB) 14.9 ms -> 1.63 ms; wall clock, main `ab643e7` against the branch, interleaved, `b200-items` -33.5 / -1.5 / +4.9 / -26.0 ms in four batches of 15-41 pairs (the VM spreads +-12 ms between identical binaries), `01-basic` and `02-long` +-0.5 ms. (b) The Blitz document and the drawables dropped on a rayon worker were byte-identical but not faster (document +64 / +80 ms, drawables -1 ms), so the fork change was reverted; leaking them would save 41-57 ms (`docs/postmortems/2026-10-27-f99f-offthread-drop.md`). `b200-items` is 1.2-1.3 s here, still over the goal; the margin-box documents (M1) and the inline measurement memo (L1) are next. Plan: `docs/plans/2026-10-27-f99f-main-thread-leftovers.md` §7.
- The very first render of a large document can be much slower than the rest (`b200-items` took 5.7 s once, then 2.2-2.5 s; cause not investigated, probably cold caches); that is why gated documents get a warm-up.

## The guard (`--check`)

Applies to gated documents (fixtures and `b200-*`). A document fails when its median exceeds

- the **hard budget**, on any machine: 100 ms for a fixture, 1500 ms for a `b200-*`; `b200-items` is 3000 ms because it takes about 1.6 s
  (`HARD_MS_OVERRIDE` in the script, 6000 ms until F99b; lower it in the PR that makes that shape faster), or
- **baseline x 1.30 + 5 ms**, only when the machine label (`nproc` + CPU model) equals the one in `bench/baseline.json`. A CI runner has a
  different label, so there only the hard budgets apply until a `bench/baseline-ci.json` exists (plan §4 step 4: commit it from the first CI
  artifact).

Separately, the **scaling ratio** of variant `t` must not exceed `SCALING_MAX_RATIO` (see "Scaling of one long table"); a full `--check` run does it at the end.

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

Library-level timing (no process start), peak memory (F99a and F99b measured it with `resource.getrusage` of the child: `/usr/bin/time` is not installed on the dev VM; 128-243 MB for the b200 documents, 243 MB `b200-items` before and 243 MB after F99b), multi-threaded callers, and anything on CI hardware
until the first CI run has produced numbers.
