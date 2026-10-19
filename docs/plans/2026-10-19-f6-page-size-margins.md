# F6 (`@page` half): the document's `@page` size and margin decide the page

Date: 2026-10-19. Finding: F6, the `@page` paragraphs only (`2026-10-15-m0-findings.md:85-98`). Gradients, `shadow-*` and `ring-*` are the other half and are not covered here. Status: planned.
The engine part goes on the fulgur fork (`subapavel/fulgur` `htmltopdf/srgb-colours`, after `c2d07c9`). The policy part goes in `crates/htmltopdf`. Line references are to `/home/user/fulgur` and to this repo as I read them; I could not confirm the fork commit with git.

## 1. Problem

`Renderer::engine` always calls `.page_size(PageSize::custom(..))` and `.margin(Margin::uniform_mm(..))` (`crates/htmltopdf/src/lib.rs:382-385`). In fulgur both calls set an override flag (`config.rs:187-198`). An override beats every `@page` rule (`gcpm/page_settings.rs:92-97`, `:142-149`). So whatever the document's `@page` says, the page is A4 portrait with 15 mm margins. The CLI makes the same choice: `--margin-mm` defaults to 15 and `--landscape` forces A4 landscape (`htmltopdf-cli/src/main.rs:36-41`, `:107-113`).

Chrome honours `@page`. Our references are printed with `preferCSSPageSize: true` (`scripts/reference/render.mjs:41-42`).

- **Fixture 05-fancy (`@page { size: A4; margin: 0 }`) shows the gap today.** Our header band starts 15 mm in from each edge, while Chrome's is full-bleed. The `compare.py` comment blames F6 (`scripts/compare.py:43-45`); worst page 21.6 % (F102 §7.3).
- **Fixture 02-long is quietly affected too.** Its `margin: 15mm 15mm 20mm 15mm` loses its 20 mm bottom, so its rows and footer sit against a different content box than Chrome's.
- **Page size is ignored as well.** `@page { size: A5 }` and `@page { size: A4 landscape }` both give a 595.28 x 841.89 pt A4 portrait page.
- **The engine has a second gap.** Even with no override, fulgur maps only `A4`, `A3` and `letter`. Every other keyword silently becomes A4 (`page_settings.rs:6-13`). `size: landscape A5` (orientation first) is rejected as trailing tokens (`parser.rs:310-319`, `:366`).

## 2. Spec

There is no `css-spec-researcher` output. Every reference below is from memory and **unverified**.

- **css-page-3, "Page context" / `@page` rule.** Page size and margins are properties of the page context, set by the author's `@page` rules. They cascade like other properties: the author's rules beat the UA default.
  - `:first`, `:left` and `:right` rules beat the plain `@page` rule.
  - `margin` takes 1 to 4 values, and the `margin-*` longhands work as in CSS 2.1 §8.3.
- **css-page-3, "Page size: the `size` property".** Grammar: `<length>{1,2} | auto | [ <page-size> || [ portrait | landscape ] ]`.
  - A single length makes a square page.
  - `landscape` puts the long side horizontal.
  - Keyword table: A5 148x210 mm, A4 210x297, A3 297x420, B5 176x250, B4 250x353, JIS-B5 182x257, JIS-B4 257x364, letter 8.5x11 in, legal 8.5x14 in, ledger 11x17 in.
  - An unknown keyword makes the declaration invalid, so it is ignored (CSS Syntax 3).
- **Default margins are left to the UA.**
  - Chrome prints about 1 cm (0.4 in) when nothing is set.
  - WeasyPrint's UA sheet uses 75 px.
  - Our current 15 mm is an equally valid UA choice.
- **Caller settings.** A setting the caller passes explicitly works like a print-dialog choice. Chrome lets it win unless `preferCSSPageSize` is set. We let the document win by default and the caller win only when the caller sets something.

## 3. Where it belongs and the decision

| Option | What | Decision |
|---|---|---|
| A | Ours only: stop forcing size and margin | Rejected. A5 and the other keywords still become A4, and documents without `@page` silently move from 15 mm to fulgur's default 20 mm (`config.rs:70-74`) |
| B | Ours injects a UA stylesheet `@page { margin: 15mm }` first in the bundle | Rejected. It depends on the bundle/link/inline merge order of `parse_gcpm` (`engine.rs:193`, `:227`, `:235`), and it still needs the fork for the keywords |
| **C** | Fork: complete keyword table, orientation in either order, and a non-override "config default" setter. Ours: optional size and margin | **Chosen.** fulgur's priority model already has a "Config defaults" layer (`page_settings.rs:45-49`); it only lacks a public setter |

### Fork (`htmltopdf/srgb-colours`)

- **Keyword table.** `fn keyword_to_page_size(name: &str) -> Option<PageSize>` (`page_settings.rs:6`) covers the full css-page-3 list.
  - `A4`, `A3` and `letter` keep the existing constants (`config.rs:9-20`), so that the roughly 40 fork goldens and snapshots that contain 595.28 / 841.89 stay byte-identical.
  - The new keywords are built with `PageSize::custom(mm)` or from inches x 72.
- **Parser.** `parse_page_size_value` (`parser.rs:304`):
  - accepts `<orientation> <keyword>` as well as `<keyword> <orientation>`;
  - returns `None` for an unknown keyword, so the declaration is dropped and an earlier valid `size` still applies.
- **Default margin without override.**
  - `ConfigBuilder::default_margin(self, m: Margin) -> Self` (`config.rs:187`) and `EngineBuilder::default_margin(self, m: Margin) -> Self` (`engine.rs:1335`).
  - They set `config.margin` and leave `overrides.margin` false, so the margin cascade at `page_settings.rs:145-148` starts from it.
- **Per-side margins.** `Margin::sides_mm(top: f32, right: f32, bottom: f32, left: f32) -> Margin` (`config.rs:65`).
  - Reason: our crate must not do mm→pt conversion itself (AGENTS.md units invariant). Today it uses `Margin::uniform_mm`, which only takes one value.

### Ours (`crates/htmltopdf/src/lib.rs:145-193`, `:382-385`)

- **Public types.**
  - `pub struct PageSize { pub width_mm: f32, pub height_mm: f32 }`, with constants `A3`, `A4`, `A5`, `LETTER`, and `fn landscape(self) -> Self`. `landscape` puts the long side horizontal and is idempotent, not a swap.
  - `pub struct Margins { pub top_mm: f32, pub right_mm: f32, pub bottom_mm: f32, pub left_mm: f32 }`, with `fn uniform_mm(mm: f32) -> Self`.
  - `pub struct Page { pub size: Option<PageSize>, pub margin: Option<Margins> }`, derived `Default` (both `None`), and `pub const DEFAULT_MARGIN_MM: f32 = 15.0`.
  - `RenderOptions::page` keeps its name and gets the new type. The crate is unpublished and has no bindings (`crates/` holds only the library and the CLI), so the break costs nothing.
- **Precedence**, applied per side and per property:
  1. what the caller set;
  2. the document's `:first` / `:left` / `:right` rules;
  3. the document's plain `@page` rule;
  4. the default: fulgur's `PageSize::A4` portrait and 15 mm.
- **Wiring.**
  - `size = Some(s)` → `.page_size(PageSize::custom(..))`. This is forced, orientation included, as today.
  - `size = None` → no call. fulgur's default `PageSize::A4` (`config.rs:111`) is then the same page box as `@page { size: A4 }`, so a document without `@page` and every fixture get the same A4.
  - `margin = Some(m)` → `.margin(Margin::sides_mm(..))`.
  - `margin = None` → `.default_margin(Margin::uniform_mm(15.0))`.
- **Why 15 mm.** It is the product's current behaviour, and about 30 tests in `crates/htmltopdf/tests/` assume it for documents that have no `@page`.
- **CLI.**
  - `--page-size <A3|A4|A5|letter|WxH>` (WxH in mm) is optional.
  - `--landscape` forces the `--page-size` value, or A4 when there is none, in landscape. That is today's meaning.
  - `--margin-mm <f32>` becomes `Option` with no default.
  - With no page flag, the document decides. `scripts/render-fixtures.sh` and `scripts/bench.py:389-398` pass no page flags, so they need no change.
- **ADR-0005** records the precedence and the 15 mm default. Reason: it is public-API behaviour that is hard to change once bindings exist.

## 4. Acceptance

- **`crates/htmltopdf/tests/page_from_css.rs` (new).** Helpers: `media_boxes(pdf)` (`pdfinfo -box`) and `words(pdf)` (`pdftotext -bbox`, as in `body_offset.rs`). Tolerance ±0.1 pt.

  | Test | Input | Expected | Red before |
  |---|---|---|---|
  | T1 | `size: A5` | MediaBox 419.53 x 595.28 | yes |
  | T2 | `size: A4 landscape` | 841.89 x 595.28 | yes |
  | T2 | `size: landscape A5` | 595.28 x 419.53 | yes |
  | T3 | `size: 210mm 99mm` | 595.28 x 280.63 | yes |
  | T3 | `size: 5in` | 360 x 360 | yes |
  | T4 | `size: A5` followed by `size: Foo` | stays A5 | yes |
  | T5 | `@page { margin: 0 }`, `body { margin: 0 }` | first word x0 < 1 pt, top < 3 pt | yes |
  | T6 | `margin: 10mm 20mm 30mm 40mm` | x0 = 113.39 pt; the last line of a multi-page text ends at or above page height − 85.04 pt | yes |
  | T6 | `margin: 10mm; margin-left: 25mm` | x0 = 70.87 pt | yes |
  | T7 | `size: A5`, caller `size: Some(A4)` | A4 portrait | no |
  | T7 | `size: A5 portrait`, caller `Some(A4.landscape())` | A4 landscape | no |
  | T8 | `margin: 0`, caller `Margins::uniform_mm(15)` | x0 = 42.52 | no |
  | T9 | no `@page` | 595.28 x 841.89, x0 = 42.52 | no (guard) |
  | T10 | only `size: A5` | margin stays 15 mm | yes |
  | T10 | only `margin-top: 30mm` | left margin stays 15 mm | — |
  | T11 | `size: A5; margin: 10mm` with `@bottom-center` | its text sits inside the bottom 28.35 pt band of an A5 page | — |
  | T12 | `@page :first { margin-top: 50mm }`, 3 pages of words | every word once, none below its page's bottom margin edge | — |
  | T12b | the reverse: `:first` smaller than the other pages | `#[ignore = "F109"]` | — |
  | T13 | `@page cover { size: A5 }` (named page) | page stays A4. From reading cssparser, the whole rule is dropped; step 1 records it | — |

- **CLI: `crates/htmltopdf-cli/tests/page_flags.rs` (new).**
  - An A5 document with no flags gives A5.
  - `--page-size A4` gives A4.
  - `--landscape` alone gives A4 landscape.
  - `--margin-mm 0` on a 15 mm document gives x0 < 1.
- **Fork tests.**
  - Lib tests for the keyword table, both orientation orders, the invalid keyword, and `default_margin` (the `overrides.margin` flag stays false; the CSS margin wins).
  - `render_smoke`: an A5 document gives an A5 MediaBox.
  - Expected golden movers: exactly the five VRT goldens that declare `size: A5` and were rendered as A4: `gcpm/margin-box-implicit-href`, `toc-target-counter`, `toc-target-counter-literal`, `leader-toc`, `bookmark-label-counter`.
    - Re-bless one only after `pdfinfo` shows 419.53 x 595.28.
    - Any other mover stops the step.
- **Existing tests to port.**
  - `table_columns.rs:21`, `:32-34` assumes 15 mm despite its own `@page { margin: 0 }`: drop the offset.
  - `running_elements.rs:14`, `:23`, `:350`: take the band from the document's 25 mm / 15 mm.
  - Mechanical moves to the new `Page` shape: `cell_vertical_align.rs:327-338`, `row_backgrounds.rs:174-184`, `list_markers.rs:282`, `table_fixup.rs:461`, `img_natural_size.rs:358`, `wrapper_line_split.rs:741-758`, `image_robustness.rs:459`.
  - Each "`Page::margin_mm` wins over `@page`" comment is corrected.
- **Fixtures** (`scripts/render-fixtures.sh`, then `compare.py`, then look at `out/compare/*.png`).
  - **05-fancy.** The PNG must show the header gradient from x = 0 to 794 px and from y = 0, with text at x ≈ 57 px (`px-[15mm]`), as in Chrome.
    - Worst page must drop from 21.6 %. Step 0's `--margin-mm 0` render predicts the number.
    - Threshold: new worst + 1 point. Rewrite the comment at `compare.py:43-45`.
  - **02-long.** Bottom margin goes 15 → 20 mm (19 px less body per page). The footer is centred in a 20 mm band like Chrome's.
    - Page count must stay 3/3: 02 is not in `PAGE_COUNT_MAY_DIFFER`.
    - Per-page numbers equal or lower. Lower the threshold if they improve.
  - **01, 03, 04, 06-17.** These declare `size: A4; margin: 15mm`. They are **not** expected byte-identical, for two reasons:
    - `size: A4` gives `PageSize::A4` = 595.28 pt, where our forced `PageSize::custom(210, 297)` gave 595.2756;
    - CSS `15mm` is computed as `15 * (72/25.4)` (`parser.rs:290`), `Margin::uniform_mm` as `15*72/25.4` (`config.rs:66`), and the two can differ in the last f32 bit.
    - Gate: same page counts; same `pdftotext -bbox` words within ±0.05 pt against main; `compare.py` within ±0.1 point.
    - 17 (multi-line `@page` with a margin box) has the same values, so the same gate applies.
  - **New `18-a5-receipt`.** The reason it exists: the invariant demands a fixture that shows the size change, and A5 is what Czech shops print simplified tax documents and cash receipts on.
    - Content (Czech): "Zjednodušený daňový doklad č. 2026-0018", seller with IČO / DIČ, date of issue / DUZP, about 6 item rows, a 21 % / 12 % VAT summary, the total, "Zaplaceno platební kartou".
    - Page rule: `@page { size: A5; margin: 10mm; @bottom-center { content: "Děkujeme za nákup · www.example.cz"; font-family: Inter, sans-serif; font-size: 8pt; color: #6b7280 } }`. One page.
    - Use only classes found in `fixtures/tailwind/tailwind.css` by grep (`text-xs`, `w-full`, `text-right`, `tabular-nums`, `border-b`, `justify-between`, … are present). Anything else goes in inline `style`; no npm rebuild.
    - Reference: `node scripts/reference/render.mjs 18-a5-receipt`, committed.
    - The PNG must be 559-560 x 794 px, with content 38 px from the left and top and the footer in the bottom 38 px band.
    - Threshold: worst page + 1 point.
- **Bench and determinism.**
  - `fixtures.rs` determinism stays green.
  - `bench.py --check` passes.
  - `b200-items` uses 02's 20 mm bottom (`bench.py:119-124`), so it may leave `B200_PAGES` (`:61`). If it does, retune its row count to about 200 pages, as B1 prescribes, and say so in the PR.
- **Reviews.** `vrt-validator` with no high-severity finding. `pdf-reviewer` is not required: the PDF writer path is unchanged (page size goes through the same `PageSettings::from_wh`, `render.rs:208`).

## 5. Steps (each one day or less, each testable on its own)

0. **Measure (half a day).** Main binary:
   - per-fixture `compare.py` numbers, `pdftotext -bbox` word lists and MediaBoxes for 01-17 and the bench corpus;
   - 05 rendered with `--margin-mm 0` (predicts the fix for 05).
   - **Stop** if 05 does not improve: write a postmortem and re-plan.
1. **Red tests (one day).** `page_from_css.rs`, `page_flags.rs` (against the planned signatures) and the fork lib tests. Record which are red.
2. **Fork (one day).** Keyword table, parser, `default_margin`, `Margin::sides_mm`. Then fork `fmt`, `clippy`, lib, `render_smoke`, `gcpm_*`, VRT and `examples_determinism`. Re-bless only the five A5 goldens. Push.
3. **Ours, API (one day).** Lock bump, the new `Page` / `PageSize` / `Margins`, `engine()`, port the existing tests. T1-T13 green.
4. **CLI (half a day).** The flags; `page_flags.rs` green.
5. **Fixtures (one day).** Render 01-17 and run the step 0 gate. Look at 05 and 02. Update thresholds and comments. Add fixture 18 with its README and reference.
6. **Docs (half a day).**
   - ADR-0005;
   - findings: F6 `@page` half done, F109 filed;
   - fork entry in `docs/upstream.md`;
   - crate doc (`lib.rs:8`);
   - §7 here.
   - Then `vrt-validator`.

## 6. Risks and non-goals

- **Page-1 content height on every page.** The fragmenter paginates every page against page 1's content height (`engine.rs:529-552`), while margins are resolved per page at paint time (`render.rs:195-210`).
  - A `:first` rule with a *smaller* margin than the other pages can push content on pages 2+ into the bottom margin.
  - This is reachable now because documents' margins apply. Filed as **F109**. T12 guards the common letterhead shape (a larger `:first` margin).
- **Margin 0 meets two open findings.** Full-bleed multi-page documents now reach F30c (a widow line lost at margin 0, `m0-findings.md:306-309`) and F89 (one page more than Chrome). 05 is one page. Neither is fixed here.
- **A4 in two values.** `@page { size: A4 }` gives 595.28 pt, while a caller forcing `PageSize::A4` gets 595.2756. The 0.0044 pt is accepted: making fulgur's constants exact would re-bless about 40 fork goldens. Recorded in §7.
- **Reference without `@page`.** Chrome references of documents without `@page` would use Playwright's defaults, not ours, so every fixture keeps declaring `@page`.
- **Unpublished break.** `--margin-mm` and `Page` change meaning; there are no users yet.
- **Non-goals:**
  - named pages and `page:`;
  - per-page size for `:first` / `:left` / `:right` in pagination (F109);
  - `%`, `em`, `auto` and `vh` in page margins (ignored, `parser.rs:288-296`);
  - `@page` inside `@layer` / `@media`;
  - margin boxes' vertical centring (F103);
  - the gradient / shadow / ring half of F6;
  - an orientation-only caller override.

## 7. Result

Done 2026-10-19, steps 0-6 (the `vrt-validator` run is the caller's). Fork commit `39ac1b2` on `subapavel/fulgur` `htmltopdf/srgb-colours` (after `c2d07c9`); ours: red tests `686764f`, the rest in the commits after it on `f6-page-size-margins`; `Cargo.lock` points at `39ac1b2`.

**Step 0 (main's binary `33b0519`).** MediaBox 595.28 x 841.89 and compare numbers as in the table below for all of 01-17. 05 rendered with `--margin-mm 0` (the predicted fix): 21.6 -> 3.45 %, so the plan went on. Word lists (`pdftotext -bbox`) saved for the step 5 gate.

**Red before.**
- `page_from_css.rs`: 15 of 19 tests red (T1, T2 x4, T3 x2, T4, T5, T6 x2, T10 x2, T11, T12); green guards T9, T13 and "unknown keyword alone"; T12b `#[ignore = "F109"]` (run with `--ignored` after the change it fails on page 2: `Line095` ends at 743.2 pt, the margin edge is 728.5 pt, so F109 is real). T7 / T8 (caller wins) were written with the new API in step 3.
- `page_flags.rs`: 4 of 7 red (`--page-size` does not exist, the flags forced A4 / 15 mm), 3 green guards.
- Fork: the lib tests did not compile (`default_margin`, `sides_mm`, the `Option` keyword table); the three `render_smoke` tests gave an A4 MediaBox.

**Numbers (fulgur vs Chrome, fraction of pixels, worst page last column).**

| Fixture | pages main -> now | per page before | per page after | threshold |
|---|---|---|---|---|
| 01 | 1 -> 1 | 4.4 | 4.4 | 5 % (unchanged) |
| 02 | 3 -> 3 (Chrome 3) | 10.2 / 11.4 / 9.8 | 10.2 / 11.1 / 9.5 | 12.4 -> 12.1 % |
| 03 04 06-16 | unchanged | unchanged | unchanged to 0.1 point | unchanged |
| 05 | 1 -> 1 | 21.6 | **3.5** | 22 -> 4.5 % |
| 17 | 2 -> 2 | 12.0 / 10.0 | 12.0 / 10.0 | 13 % |
| 18 (new) | - -> 1 (Chrome 1) | - | 5.3 | 6.3 % |

- **05** (PNG looked at): the header gradient now runs from x = 0 to 794 px and from y = 0 like Chrome's, text at x ~ 57 px, the cards and table line up with Chrome's; what is left is text drift and the shadow band under the totals box.
- **02** (page 2 looked at): the 20 mm bottom margin applies, 16 words move from page 2 to page 3 (359 / 369 / 280 -> 359 / 353 / 296 words), the footer is in the 20 mm band; it is not centred vertically in it as in Chrome (F103).
- **01, 03, 04, 06-15:** words within 0.0000 pt of main (the two A4 widths and the two `15mm` conversions turned out equal after rounding to the PDF's precision), compare numbers equal. 16 / 17: words within 0.0002 pt.
- **18:** PNG 560 x 794 px, content 38 px from the left and top, footer in the bottom 38 px band (Chrome's footer is centred, ours starts at the margin edge and is about 11 px higher: F110, F103). MediaBox ours 419.53 x 595.28, Chrome's 420 x 594.96 pt.
- **Bench corpus:** `b200-text` 200 pages and `b200-mixed` 201, words within 0.00006 pt of main; `b200-items` 200 -> **204 pages** (its `@page` declares 15 / 20 mm, now honoured): still inside `B200_PAGES` (195-205), so no retuning. `bench.py --check` ok (02-long 41.8 ms vs main's binary 40.9 ms, same run; 18: 20 ms; the new fixture has no baseline entry and is held by the 100 ms hard budget).

**Tests.** Fork: `cargo test -p fulgur --lib` 2175 passed, every integration test target green, `--doc` green, `fulgur-vrt` green **without** re-blessing, `fmt --check`, `clippy -p fulgur --all-targets` clean; fulgur-cli `examples_determinism` 10 passed, 1 failed (`committed_svg_matches_rendered`, the same failure on `c2d07c9`: the SVG example needs host fonts). Ours: `cargo test --release --no-fail-fast` 495 passed, 0 failed, 4 ignored (incl. T12b); `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

**Deviations from the plan.**
1. **No VRT golden moved.** The plan expected five (`size: A5` documents rendered as A4). `fulgur-vrt`'s manifest sets `[defaults] page_size = "A4"`, an override, so the harness never sees the document's size. Nothing re-blessed; the fork's own `render_smoke` tests cover A5.
2. **Existing tests that depended on the override** besides the ones the plan names: `print_media.rs` (its bundled `@page { margin: 0 }` now applies; the sampling column moved from x = 120 to 50 px at 48 dpi). `running_elements.rs` got a `Band` (15 mm default, 25 mm for R1 / R7) instead of one constant.
3. **Test details.** The CLI reports a bad `--page-size` / negative `--margin-mm` as an error (not in the plan). T11 pins the margin box's x at the left margin edge instead of "centred" because of F110. T5 / T10 allow the glyph box to start 2 pt above the line (83 pt for a 30 mm margin).
4. **Fork tests were run with the pinned fontconfig minus the system Inter** (`FONTCONFIG_FILE` with a `rejectfont` for `/usr/share/fonts/*/inter`, the setup earlier fork work used on this VM), not the bare `examples/.fontconfig/fonts.conf`, and with the local Blitz checkout through `--config patch.crates-io` (never committed; the fork's `Cargo.lock` was restored each time).
5. **Filed:** F109 (as planned), **F110** (a centre margin box is not centred) and **F111** (bare text in `<body>` taller than a page loses its tail silently, on main too; HIGH, unrelated to F6).

**Not verified.** The CSS spec statements in §2 are still from memory (no `css-spec-researcher` run). Chrome's default margin and WeasyPrint's were not measured. `pdf-reviewer` was not run (not required). `vrt-validator` not run (the caller does). The fork was not run against CI's fontconfig, and `cargo test -p fulgur` as one command was not run (each integration target separately, to save disk).
