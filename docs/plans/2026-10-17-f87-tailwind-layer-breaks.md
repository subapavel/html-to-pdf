# F87 — Tailwind break utilities inside `@layer` never reach fulgur's break / column reader

Date: 2026-10-17. Finding: F87 in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/fulgur`
`htmltopdf/srgb-colours` (after `752df13`), carried via `[patch.crates-io]`. All fork line numbers are at `752df13`.

## 1. Problem

`<p class="break-inside-avoid">` with the compiled `fixtures/tailwind/tailwind.css` is an ordinary paragraph. `style="break-inside:avoid"` works. So
does a plain `<style>` rule (probes `/tmp/claude-0/vrt-f83/cls/`; the first version of fixture 13 was byte-identical on main and on the F83 branch,
see the F83 plan §7.2).

Pagination reads `break-*` only from the fork's side table: `pagination_layout.rs:1184-1188`, `:1260-1264` and `:2773-2776`.
`multicol_layout.rs:219-246` reads `column-fill`, and `blitz_adapter.rs:866`, `:915` read `page`. Stylo (servo) does not compute these properties
(`column_css.rs:1-4`). `extract_column_style_table` (`blitz_adapter.rs:712-728`) fills that table. It joins every `<style>` (`:1130-1173`) and passes
the result to `parse_stylesheet` (`column_css.rs:929-940`). The parser's `AtRuleParser::parse_prelude` returns `Err` for every at-rule (`:951-960`),
so cssparser drops the whole block. Tailwind v4 puts every utility inside `@layer utilities{…}` (`tailwind.css:2`), so the reader never sees one.
`column_css.rs:1505-1515` pins the skipping for `@media`.

What is in the compiled sheet (grep of `tailwind.css:2`):

- At-rules: `@layer properties{@supports(…){*,:before,:after,::backdrop{--tw-…}}}`, `@layer theme{…}`, `@layer base{…}` (with two `@supports`
  blocks), the statement `@layer components;`, and `@layer utilities{…}`. Inside utilities are three `@supports (color:color-mix…)` blocks,
  `@media (hover:hover){…}` and `@media print{.print\:bg-slate-900{…}}`. Then come `@font-face` and `@property`.
- Declarations the reader parses (`break-*`, `page-break-*`, `page`, `column-fill`, `column-rule*`): exactly one,
  `.break-inside-avoid{break-inside:avoid}`, in `utilities`. There is no `break-before-page`, no `break-after-page`, no `print:break-*` and no
  `columns-*`.
- `columns-*` is **not** part of this finding. `column-count` and `column-width` come from Stylo (`blitz_adapter.rs:1519-1544`), which handles
  layers. Step 1 confirms this.

Fixtures using the class: only 02, on 69 body `tr`s (`index.html:61`…`:474`). Its own `<style>` already sets `tr { break-inside: avoid; }` (`:22`),
so the class adds nothing. Fixture 13 sets the property inline because of this finding (`README.md:15-17`).

## 2. Spec (cited from memory, unverified; css-spec-researcher not run)

- **CSS Cascade 5 §6.1, cascade sort order:** origin and importance, then context, then element-attached styles (the `style` attribute), then
  cascade layers, then specificity, then order of appearance.
- **CSS Cascade 5 §6.4, cascade layers:**
  - Normal declarations: an earlier-declared layer loses to a later one.
  - Unlayered styles form an implicit final layer and beat every layer, whatever the specificity.
  - A layer's own rules (outside any sub-layer) are an implicit final sub-layer, so they beat that layer's sub-layers.
  - `@layer a.b {}` is the same layer as `@layer a { @layer b {} }`.
  - `@layer a, b;` declares the order without rules. Each anonymous `@layer {}` is a distinct layer.
  - Order is by first appearance across the whole document.
  - Whether a `@layer` inside a conditional rule that does not apply declares a layer: not recalled. No fixture depends on it.
- **Media Queries 4 §2.2–2.3:** media types `all`, `print`, `screen`, with the `not` and `only` prefixes. CSS Conditional 3 §5 (`@media`) and §6
  (`@supports`).
- **CSS Syntax 3 §4.3.7:** escapes inside an ident. `.print\:x` is the class `print:x`.
- **Tailwind v4** (seen in the compiled file): utilities are plain single-class selectors in `@layer utilities`, and `print:` compiles to
  `@media print{…}` nested in that layer. So utilities never compete by specificity among themselves.

## 3. Where it belongs and design (fulgur fork, `column_css.rs`)

The engine-level fix would be for Stylo (servo) to compute `break-*`, `column-fill` and `column-rule-*`, so the reader would go and the full
cascade would apply (specificity, `!important`, media features). We do not do that here. It needs a Stylo fork under Blitz and every consumer
rewired, which is far larger than this finding. The reader is fulgur code, so the fix goes there. Our crate changes only in tests and fixture 13.

**Q1, cascade.** The reader folds rules in source order with no specificity (`:24-25`, `:1052-1060`) and folds the inline style last
(`:1061-1066`). Descending without a layer rank would flip decisions:

| Case (`<p class="… break-inside-avoid">` near the page bottom) | Spec / Stylo | Today | Source order only | Unlayered first, then source order | Layer rank |
|---|---|---|---|---|---|
| A: head `<style>p{break-inside:auto}</style>` (unlayered, lower specificity) | auto | auto | **avoid** | auto | auto |
| B: `<body><style>@layer base{p{break-inside:auto}}</style>` (after the bundled sheet, `base` < `utilities`) | avoid | **auto** | **auto** | **auto** | avoid |
| C: head `@layer utilities, overrides; @layer overrides{.t{break-inside:auto}}`, class `t` | auto | auto | **avoid** | **avoid** | auto |
| D: `@layer a{.k{break-inside:auto} @layer b{.k{break-inside:avoid}}}` | auto | auto | **avoid** | **avoid** | auto |

The bundled sheet comes after the head `<style>` elements: `InjectCssPass` appends a `<style>` to `<head>` (`blitz_adapter.rs:2028-2060`,
`:1585-1605`) and the harvester walks the DOM in order. Splitting into unlayered then source order gets every fixture right, but B, C and D are
realistic and it fails them. The full rank costs no more at runtime, so we must use it: it is the only model that matches Stylo for the same element.

- `parse_stylesheet` keeps its signature. It returns rules in ascending cascade order: a stable sort by layer rank, so source order holds inside a
  rank. Unlayered rules get the highest rank. `build_column_style_table` and `walk` stay unchanged, so last-wins per field is the cascade.
- The inline style stays folded last, which is right: element-attached styles beat layers. Specificity stays ignored. That is safe for Tailwind,
  because every utility is a single class and loses to any unlayered rule anyway.
- Layer ranks: a tree of layers in first-appearance order. The rank is the post-order position (children first, then the layer's own rules), with
  root/unlayered last. Built with `Vec` and `BTreeMap` only (determinism).

**Q2, `@supports`: keep skipping.** Tailwind puts only custom properties and `color-mix` colour fallbacks there, never a property the reader parses.
Descending unconditionally would apply `@supports not (…)` fallbacks. Evaluating conditions means modelling which properties "fulgur" supports,
which is neither Stylo's answer (servo lacks `break-inside`) nor Chrome's. A skipped rule leaves today's value, so skipping cannot regress anything.

**Q3, `@media`: match the device Stylo uses.**

- Blitz builds the Stylo device as `MediaType::screen()` (`blitz-dom` `document.rs:173-180`). Our crate turns the author's `print` into `screen` and
  back in every `@media` prelude before fulgur sees the CSS (`media.rs:3-13`; in `<style>` via `lib.rs:254-263`, in bundled CSS via `lib.rs:440`;
  nested in `@layer` too, `media.rs:143-146`).
- So the reader must descend into **`all` and `screen` as written in the text it receives**. With our swap, that means the author's `@media
  print`/`all`/`not screen` apply and the author's `@media screen` is skipped, as for every other property. A break rule and the other declarations
  of its `@media` block must switch together. Matching `print` in the fork would apply the author's screen rules for us.
- Grammar: a comma list. A query matches when it is `[only|not]? <type>` with no `and`/feature. Any query with features (`(hover:hover)`, Tailwind
  `sm:` → `(width>=40rem)`) or a syntax the reader does not know does not match (skip; nothing changes from today).
- The `<style media>` attribute filter (`blitz_adapter.rs:1148-1157`, print semantics; our crate does not swap attributes) stays as is.

**Q4, selectors.** Break utilities are plain classes. `parse_selector_list` re-tokenises the raw prelude (`:976-979`), and cssparser unescapes idents,
so `.print\:break-before-page` becomes `Class("print:break-before-page")` (`:782-795`) and matches the class token (`:853-854`). From reading; a unit
test pins it. Variant forms with pseudo-classes or `:where()` (`first:`, `*:`) stay dropped (`:821-825`). They are not break utilities in the
compiled sheet.

Signatures:

- `const READER_MEDIA_TYPE: &str = "screen";`, with a comment naming `make_device`.
- `const MAX_AT_RULE_DEPTH: usize = 32;`. Deeper blocks are skipped. We must cap recursion because a stack overflow aborts the process and our crate's
  `catch_unwind` (`lib.rs:268-282`) cannot catch it. Invoices nest at most 3 deep.
- `struct LayerTree`, `fn child(&mut self, parent: LayerId, name: Option<&str>) -> LayerId`, `fn ranks(&self) -> Vec<usize>`.
- `fn media_query_list_matches(input: &mut Parser<'_, '_>) -> bool`.
- `SheetParser { rules, layers: &mut LayerTree, layer: LayerId, depth: usize }`:
  - `@layer` block: recurse into the block.
  - `@layer` statement: `rule_without_block` registers the names.
  - `@media` that matches: recurse at the same layer.
  - Anything else: `Err`, as today.
- `StyleRule` gains `layer: LayerId`.
- Rewrite the module docs at `:11-25` and the test at `:1505-1515`.

**Predicted output.**

- Fixtures 01 and 03–12: no class and no own at-rules with these properties, and the only rule added is `.break-inside-avoid`, so byte-identical.
- 02: same table values, so byte-identical.
- 05: `print:bg-slate-900` has no reader property and is dropped (`:998`).
- Fulgur VRT and examples have no `@media`, `@layer` or `@supports`, so byte-identical.
- Any re-bless is a regression.

## 4. Acceptance

New file `crates/htmltopdf/tests/tailwind_breaks.rs`: A4, Inter, strict, `css_file(fixtures/tailwind/tailwind.css)`, plus `TAILWIND_EXTRA` added with
`.css()`. `TAILWIND_EXTRA` is
`@layer utilities{.break-before-page{break-before:page}.break-after-page{break-after:page}.break-inside-auto{break-inside:auto}@media print{.print\:break-before-page{break-before:page}}}`:
Tailwind v4's shape, written inline because the compiled file lacks these (precedent: `white_space.rs:9`). Rebuilding needs npm, and `package.json`
has `^4.1.0` unpinned, so every fixture could drift. Assertions use `pdftotext -bbox` (the `printed_words` helper copied from
`break_inside_avoid.rs`). "Spacer" means `<div style="height:960px">`.

1. Spacer, then `<p class="break-inside-avoid">` of 100 words: every word on page 2, the same as `style="break-inside:avoid"`; without the class,
   word 0 is on page 1. **Red.**
2. `<p>a</p><p class="break-before-page">b</p>`: `b` on page 2. **Red.**
3. `<p class="break-after-page">a</p><p>b</p>`: `b` on page 2. **Red.**
4. `print:break-before-page`: `b` on page 2. **Red.**
5. Case A (`p{break-inside:auto}` in head plus the class): word 0 on page 1. Green today; must stay green (a naive descent turns it red).
6. `class="break-inside-avoid" style="break-inside:auto"`: page 1. Green guard.
7. Case B (body `<style>@layer base{p{break-inside:auto}}</style>` plus the class): page 2. **Red.**
8. Document `<style>@media screen{.x{break-before:page}}</style>`: no break (green guard). With `@media print`: break. **Red.**

Fork tests in `column_css.rs`:

- layered rule read;
- cases A, C, D on the table;
- `@layer a.b` ≡ nested;
- anonymous layers distinct;
- statement-only `@layer` order;
- inline beats layered and unlayered;
- media: `screen`, `all`, `not print`, `only screen` read; `print` and `screen and (min-width:1px)` skipped;
- `@supports` skipped;
- the escaped class matches;
- 10 000 nested `@layer{` does not panic.

In `blitz_adapter.rs` tests, `column_reader_media_type_agrees_with_stylo_device`: `@media screen{#a{color:rgb(1,2,3);break-before:page}}` and the same
for `print`/`#b`. Stylo colours `#a` only, and the table has `#a` only. If someone gives Blitz a print device, this test fails and names the constant.
In `pagination_layout.rs`, a lib test (`run_pass_with_break_styles`): `@layer u{.b{break-before:page}}` on the second `<p>` gives two pages. No
`render_smoke` test is needed: the reader is pure and reached through `extract_column_style_table` in lib tests.

Pixel diff: fixtures 01–12 `cmp` byte-identical to main's binary, compare numbers and thresholds unchanged. **Fixture 13** switches to
`<p class="text-xs break-inside-avoid">` (`index.html:60`, README `:3`, `:15-17`). This proves F87 with the invoice idiom:

- On main the class version differs (the terms are an ordinary paragraph below the total; step 5 records the pages).
- With F87 it is `cmp`-identical to today's inline render: 3 pages, 1.9 / 11.7 / 1.9 %, threshold 0.127 unchanged.
- PNG: the terms start at the top of page 2 like Chrome's.
- The inline case stays covered by `break_inside_avoid.rs`.

## 5. Steps

1. **Confirm** (scratch: our CLI on the local fork, with temporary logs removed afterwards):
   - re-run `/tmp/claude-0/vrt-f83/cls/` at `752df13`;
   - log the rules `parse_stylesheet` returns for fixture 13 (expect none from Tailwind);
   - log the harvested order: head `<style>`, bundled, body `<style>`;
   - `@layer utilities{.columns-2{columns:2}}` gives two columns today (else file F87c, do not fix here);
   - check whether `break-inside:avoid !important` is dropped (`parse_entirely`, `:614`);
   - time `extract_column_style_table` on fixture 02;
   - Chrome probes (Playwright, A4, 15 mm, Inter) of cases A–D and acceptance 8.

   If the class still does nothing after a scratch descent (another gap), or Chrome contradicts §2 on A–D, stop and write
   `docs/postmortems/2026-10-17-f87-*.md`.
2. Ours: red `tailwind_breaks.rs`. Fork: red unit, agreement and lib tests.
3. Fork: `LayerTree`, media matching, depth cap, docs and the rewritten test. Then:
   - `--lib` (the same 13 failures on crates.io Blitz, 0 with the local Blitz);
   - every `tests/*.rs` run one by one;
   - VRT and the 21 examples byte-identical;
   - fmt and `clippy --all-targets -D warnings`.
4. Ours: lock bump; `cargo test --release --locked`; fixtures 01–13 `cmp` against main's binary; compare unchanged; render time of 01 and 02 unchanged
   within noise. If the added parse costs over 1 ms, stop and plan a cache.
5. Fixture 13: the class, README, Chrome reference regenerated (expected pixel-identical), compare, look at the PNGs.
6. Docs:
   - findings: F87 done; correct the `columns-*` claim; file F87a and F87b (§6);
   - `docs/upstream.md` fork row;
   - §7.

   Then `vrt-validator`. `pdf-reviewer` is not required: the PDF writer, fonts and metadata are not touched.

## 6. Risks and non-goals

- Documents whose layered or `@media print` break rules were silently ignored now break as Chrome does. That is the fix, but customers will see it.
- The media constant mirrors a Blitz quirk. When the Blitz device becomes `print` and `media.rs` stops swapping, the constant must flip with it. The
  agreement test enforces that.
- Layers declared only inside skipped `@media`/`@supports` are not registered. The spec position is unverified; no fixture depends on it.
- The whole Tailwind sheet is now parsed on every `extract_column_style_table` call (Speed goal). Step 4 measures it.
- Non-goals:
  - **F87a:** media features and Tailwind responsive variants (`md:break-before-page`), which would need Stylo media evaluation.
  - ~~**F87b:** `!important`~~ (done in §7.1, after `vrt-validator`).
  - Specificity (also done in §7.1).
  - Evaluating `@supports`.
  - `<link>`/`@import` sheets (fulgur-s5ro) and `@import … layer()`.
  - CSS nesting.
  - `revert-layer`.
  - F83a / F83b.
  - Fixture 12's inline `columns:2` (Stylo-owned; `columns-2` not compiled).
  - Rebuilding `tailwind.css`.

## 7. Result

Landed on branch `f87-tailwind-layer-breaks`; fulgur fork `htmltopdf/srgb-colours` **`645c15e`** (on top of `752df13`); the Blitz fork is unchanged. `Cargo.lock` pins `645c15e` (one line).
Status: **done**.

**Step 1, the model held** (our CLI built against the local fork with a temporary log in `extract_column_style_table`, removed again; probes in `/tmp/claude-0/vrt-f87/cls/`, Chrome = Playwright Chromium, print media,
A4, 15 mm, Inter, `line-height: 20px`):

- Fixture 13 on the unfixed code: `parse_stylesheet` returns 0 rules (Tailwind contributes none); fixture 02: 2 rules (its own `tr` and `.keep-together`). The harvested order is the head `<style>`, our injected
  `@layer htmltopdf-property-defaults{...}` sheet, then the bundled `tailwind.css` (22 KB), as §3 said. Parsing took 115-145 us; building the table for fixture 02 82 us.
- `@layer utilities{.columns-2{columns:2}}` gives two columns today (Stylo handles layers): no F87c.
- `break-inside:avoid !important` is dropped (the declaration parser never calls `parse_important`; a throwaway unit test confirmed it): F87b is real. Chrome keeps the box together, with and without a competing inline `auto`.
- Chrome agrees with §2 on cases A, B, C, D and acceptance 8 (table below). No stop.

**Deviation found in step 1: `break-inside` cannot be seen on a body-level `<p>`** (§4 items 1, 5, 6, 7 and the Q1 table use `<p>`). In the fork a body-level paragraph that does not fit below the cursor moves whole to the next page
whether or not it is `avoid` (the pre-advance of the line path, F30c; Chrome starts an `auto` paragraph on page 1 and splits it), and one that is taller than a page is relaxed to `auto` since F83. So `<p class="break-inside-avoid">`
and a plain `<p>` render the same PDF and no assertion can be red on it. The box where `avoid` and `auto` differ is a multi-column container that does not fit the room under it (F30d: `auto` splits it over the pages, `avoid`
moves it whole), and Chrome does the same. The tests use `<div class="break-inside-avoid" style="columns:2">` of 100 words after a 960 px spacer. The cases and their meaning are unchanged.

| Probe (spacer 960 px, then a two-column box of 100 words) | Chrome: page of `word00000` | ours before F87 | ours with F87 |
|---|---|---|---|
| no property / inline `avoid` | 1 / 2 | 1 / 2 | 1 / 2 |
| `class="break-inside-avoid"` | **2** | 1 | 2 |
| A: head `div{break-inside:auto}` plus the class | 1 | 1 | 1 |
| B: body `<style>@layer base{div{break-inside:auto}}</style>` plus the class | **2** | 1 | 2 |
| C: `@layer utilities, overrides;@layer overrides{.t{auto}}`, classes `break-inside-avoid t` | 1 | 1 | 1 |
| D: `@layer a{.k{auto} @layer b{.k{avoid}}}` | 1 | 1 | 1 |
| class plus inline `auto` | 1 | 1 | 1 |
| `<p>a</p><p class="break-before-page">b</p>`, `break-after-page`, `print:break-before-page` (inline Tailwind shape) | `b` on page 2 | page 1 | page 2 |
| `@media screen{.x{break-before:page}}` / `@media print{...}` | no break / break | no break / **no break** | no break / break |

**Red before** (fork `752df13`): ours `tailwind_breaks.rs`, 6 of 11 red: `break_inside_avoid_class_keeps_a_box_together`, `break_before_page_class_starts_a_page`, `break_after_page_class_starts_the_next_page`,
`print_variant_class_starts_a_page`, `a_later_source_in_an_earlier_layer_loses_to_the_utility_layer` (case B), `media_screen_does_not_break_and_media_print_does`; the other 5 are guards, green before and after (control, A, C, D,
inline beats the layer). Fork lib: 11 red (`a_rule_inside_a_layer_is_read`, `an_earlier_layer_loses_to_a_later_one_whatever_the_source_order`, `a_layer_beats_its_own_sublayers`, `a_dotted_layer_name_is_the_nested_layer`,
`anonymous_layers_are_distinct_and_ordered`, `a_layer_statement_fixes_the_order_without_rules`, `media_blocks_are_read_when_the_query_matches_the_device`, `a_media_block_nests_in_a_layer_and_in_a_media_block`,
`an_escaped_class_selector_matches_the_class`, `deeply_nested_blocks_do_not_overflow_the_stack`, `nesting_up_to_the_cap_is_read`) plus `column_reader_media_type_agrees_with_stylo_device` (red on the reader's
assertion; the Stylo colouring assertions passed, so the device is a screen) and `pagination_layout::break_before_inside_a_layer_starts_a_page` (0, 0 against 0, 1). Green guards: `unlayered_rule_beats_a_layer_whatever_the_order`
(case A), `the_style_attribute_beats_layered_and_unlayered_rules`, `supports_blocks_stay_skipped`, `other_at_rules_stay_skipped_and_do_not_swallow_the_next_rule`. Two tests were added after the first run:
`layer_ranks_are_post_order`, `stylesheet_reads_matching_media_and_layers_in_cascade_order`; `stylesheet_skips_at_rules` became `stylesheet_skips_unmatched_at_rules`. **Green after:** all of them.

**After:**

| Check | Result |
|---|---|
| fork `--lib` | crates.io Blitz: 2061 passed, 13 failed, the same 13 as at `752df13`; local Blitz (`/home/user/blitz`, `52b22ef`): 2074 passed, 0 failed |
| fork `render_smoke` | crates.io Blitz: 250 passed, 3 failed (the same `outside_*_markers`, `table_parts_with_a_non_table_display`); local Blitz: 253 passed, 0 failed |
| fork other test binaries | all 42 `tests/*.rs` one by one with the local Blitz: green |
| fork `fulgur-vrt` | green, no golden changed. It needs `FONTCONFIG_FILE` to hide the host's system Inter (`fonts-noinInter.conf`, as before); with the repo's own `examples/.fontconfig/fonts.conf` 26 of 64 differ in this environment, which is a host-font effect, not this change |
| fork examples | all 21 PDFs rendered with the debug CLI at `752df13` and after: byte-identical |
| fork fmt, `clippy -p fulgur --all-targets -D warnings` | clean |
| ours `cargo test --release --locked` | 336 passed, 0 failed (11 new in `tailwind_breaks.rs`); `cargo fmt --check`, `clippy --release --locked --all-targets -D warnings` clean |
| fixtures 01-13 | `cmp` byte-identical to the render of main's binary (`htmltopdf-main-089675f`, fulgur `752df13`): 13 of 13; `compare.py` numbers unchanged: 4.4, 10.2/11.4/9.8, 5.7, 5.7, 21.6, 15.9/11.8, 7.2/7.1, 13.2/12.1, 5.1/4.9/0.7, 5.9/1.5, 6.9, 14.6/7.2, 1.9/11.7/1.9 %; page counts and thresholds unchanged |
| render time, 5 runs x 2 rounds, main / branch | 01: 22-27 / 19-25 ms; 02: 52-58 / 49-58 ms. Within noise; the parse of the whole Tailwind sheet is about 0.1 ms (step 1 log), far under the 1 ms bound |

**Fixture 13, main against the branch: byte-identical, and so are the class, the inline style and neither property** (all six combinations of main / branch and class / inline / no property render the same PDF, 3 pages, 1.9 / 11.7 / 1.9 %,
Chrome reference pixel-identical to the committed one). §4 predicted that the class version differs on main; it does not, for the reason in the deviation above (an `avoid` paragraph and an `auto` one are placed alike at body
level, and an `avoid` one taller than a page is relaxed since F83). Fixture 13 now uses `class="text-xs text-gray-600 break-inside-avoid"` (`text-gray-600` kept, §4 dropped it from the quoted class list, but it changes the colour
and `cmp`-identity needs it); it documents the idiom but proves nothing about F87. The proof is `tailwind_breaks.rs`. The README says so.

Deviations from the plan:

1. **Tests use a two-column box, not a `<p>`** (above). Case A-D semantics and the acceptance numbers are the plan's; `p{...}` selectors became `div{...}`.
2. **Fixture 13 does not show F87** (above); the plan's step 5 expectation "differs on main" was wrong.
3. **Chrome reference** rendered with the scratch `render-ref.mjs` (hard-coded Playwright path, as in F83 §7); `chrome.pdf` bytes changed, the three PNGs are identical.
4. `media_query_list_matches` returns `true` for an empty list (Media Queries 4 §2.1); a query naming a type other than `all`, `screen`, `print` never matches, `not` included.
5. Fork `--lib` tests count 2074 (the plan did not give a number).

Filed in `2026-10-15-m0-findings.md`: **F87a** (media features, `md:break-before-page`); the first version also filed **F87b** (`!important`), which §7.1 fixed; F87c as named here (`columns-*`) was not needed. The `columns-*` claim of the F87 entry is corrected there. Added to **F30c**: a body-level `avoid`
paragraph is placed like an `auto` one.

Not verified: `vrt-validator` (not run); `pdf-reviewer` (not required, writer path untouched); the spec citations in §2 (from memory; Chrome probes agree on every case, the spec text was not read); `@layer` inside a skipped
`@media` (not registered, unverified against the spec, no fixture depends on it); `@import ... layer()`, `<link>` sheets (fulgur-s5ro); fork `fulgur-vrt` with the repo's own fontconfig (host fonts differ here); the F83
`render_smoke`-style end-to-end tests on a Tailwind page with the class (no such shape is observable on a `<p>`, see above); right-to-left or non-Latin documents (the change is CSS-only).

### 7.1 After `vrt-validator` (no-merge): `!important`, specificity, layer-name whitespace

The validator found three defects the first version introduced or exposed (fork `3b158b6` on top of `645c15e`; ours: lock bump and tests). §3's "Specificity stays ignored" and §6's F87b non-goal were wrong for a
sheet that is now read: with layered utilities visible, an important override and a more specific rule in the same layer flipped decisions that main (which dropped the whole layer) and Chrome agreed on.

1. **`!important` (HIGH).** `class="break-inside-avoid break-inside-auto!"` (Tailwind's `!` modifier: `@layer utilities{.break-inside-auto\!{break-inside:auto!important}}`) landed on page 2 where Chrome and main gave
   page 1. The declaration parser now takes the flag per declaration (`scan_tail`, `Declared { normal, important }`; `!`, optional whitespace, `important` in any case, anything else after a `!` drops the declaration)
   and `StyleRule` carries both halves. `walk` folds per element, lowest to highest: normal by layer rank, specificity, order; the `style` attribute's normal declarations; important by layer in **reverse** rank
   (Cascade 5 §6.4.4: the unlayered ones lose, an earlier layer wins, a sub-layer beats its parent), specificity, order; the `style` attribute's important declarations. Chrome probes (`/tmp/claude-0/vrt-f87/c10.py`,
   box after a spacer, same on `break-inside` and `break-before`) agree on every tier: inline important beats important unlayered, layered and earlier-layer rules; an important rule beats the inline normal; a layered
   important beats an unlayered important; an unlayered important beats a layered normal; earlier layer beats later for important in both source orders; a sub-layer beats its parent; a malformed tail
   (`!important foo`, `!`, `!importantx`) drops the declaration.
2. **Specificity (MEDIUM).** `@layer u{div.k{break-inside:auto} .k{break-inside:avoid}}`: Chrome page 1, first version page 2. The sort key is now (importance tier, layer rank, specificity, source order). Specificity
   is (ids, classes, types) of the reader's selector subset, summed over `A + B`, and for a selector list the highest of the selectors that matched this element (probes `spec_list_max`, `spec_list_max2`). The same flaw
   existed for unlayered rules on main (`unl_spec`: main 2, Chrome 1); it is fixed too. No fixture moved: fixtures 01-13 `cmp` byte-identical to main's binary, compare numbers unchanged.
3. **`@layer a .b{}` (LOW).** The dotted-name parse skipped whitespace before the dot; Chrome rejects the at-rule. It now reads the tokens including whitespace, so `a .b`, `a. b` and `a . b` make the at-rule
   (and a statement) invalid.

**Red before** (against `645c15e`): fork lib, 7 new tests: `important_declarations_are_read`, `important_beats_normal_whatever_the_layer_or_the_style_attribute`, `important_reverses_the_layer_order`,
`important_in_the_style_attribute_beats_everything`, `specificity_orders_rules_inside_one_layer_and_unlayered`, `id_beats_classes_and_types_and_adjacent_siblings_add_up`,
`whitespace_inside_a_dotted_layer_name_is_invalid`. Ours `tailwind_breaks.rs`, 7 new tests, all red: `important_modifier_beats_the_plain_utility`, `important_modifier_in_a_print_variant_beats_the_plain_utility`,
`important_modifier_cancels_a_forced_break`, `important_rule_beats_the_style_attribute_and_loses_to_its_important`, `specificity_orders_rules_inside_a_layer`, `specificity_orders_unlayered_rules`,
`whitespace_in_a_layer_name_drops_the_block`. `TAILWIND_EXTRA` gained `.break-inside-auto\!`, `.break-before-auto\!` and `print:break-inside-auto!` in Tailwind's compiled shape. **Green after:** all.

**Validator harness** (`/tmp/claude-0/vrt-f87/c1.py`-`c9.py`, each case in `break-inside` and `break-before` mode, Chrome / main / branch; ours built from the lock): mismatches 47 -> 33 and regressions
(branch differs from Chrome where main matched) 13 -> 5. `c1` 10 -> 0 (+ 4 -> 0), `c5` 2 mismatches (unchanged) and its regression 1 -> 0, `c9` 3 -> 0 (+ 3 -> 0). My own `c10.py` (important tiers, specificity, Tailwind `!`):
26 mismatches and 7 regressions -> 0 and 0. `fuzz.py 1 60`: 60 cases, 0 bad. Left, all filed: 5 regressions F87c (`avoid-page`), F87d (`media="not print"`), F87e (`<template>`, `<noscript>`); 33 mismatches the same
on main (30 media features and responsive variants, F87a; `cdc_in_middle`, `sel_list_bad`, `template_unl`).

**After:**

| Check | Result |
|---|---|
| fork `--lib` | crates.io Blitz 2068 passed, the same 13 failed; local Blitz 2081 passed, 0 failed |
| fork `render_smoke` | crates.io Blitz 250 passed + the same 3 failed; local Blitz: all 42 `tests/*.rs` including it, one by one: green |
| fork `fulgur-vrt`, 21 examples | green, no golden changed; examples byte-identical to `752df13` |
| fork fmt, `clippy -p fulgur --all-targets -D warnings` | clean |
| ours `cargo test --release --locked` | 343 passed, 0 failed (7 new in `tailwind_breaks.rs`, 18 there); `cargo fmt --check`, `clippy --release --locked --all-targets -D warnings` clean |
| fixtures 01-13 | `cmp` byte-identical to main's binary (13 of 13); `compare.py` numbers unchanged |

Filed, not fixed: F87c-F87g (findings). Not verified: the validator's 3586-level and `.a{` x 10 000 repros (taken from its report, not re-run); the speed of `walk` after the change (`hits` is built per
element and sorted; it allocates only for elements that match a rule, `perf.py` not re-run); `<style>` in `<template>` (not fixed).

### 7.2 Closing the last five harness regressions (F87c, F87d, F87e)

Coordinator's request: nothing that matched Chrome on main may stop matching. Fork `69bfd0e` on top of `3b158b6`; ours is the lock bump, the tests and the docs.

- **F87c, break values.** `break-before` / `break-after` take `auto | avoid | avoid-page | page | left | right | recto | verso | avoid-column | column` (css-break-3 §3.1; region values stay invalid). The table models a
  forced page break or none, so `avoid*` and `column` (no column breaks in the table; Chrome ignores `column` outside a multicol) map to none, and a later one cancels an earlier `page`. Chrome probes (`c11.py`, `c12.py`,
  13 values x before / after x alone / after `page`) also showed two cases the first version got wrong, both found by the same probes and fixed with it: **`break-before: always` is not a value of `break-*`**
  (Chrome ignores it and keeps an earlier `page`; the reader forced a break), and the legacy `page-break-*` takes only `auto | always | avoid | left | right` (`page`, `recto`, `verso` are invalid there; `avoid` is the alias
  of `avoid`). Three older fork unit tests pinned `break-before|after: always` as a break; they now pin it as ignored and `page-break-*: always` as a break. This moves unlayered and inline `break-before: always` toward
  Chrome too (it was forced on main).
- **F87d, `<style media>`.** The harvester evaluated the attribute as a substring test (`contains("print")`), so `not print` was print. It is now a media query list on a print device
  (`media_query_list_matches_on(input, "print", true)`; our crate swaps `print` and `screen` in CSS text, not in attributes, so the at-rule reader keeps `READER_MEDIA_TYPE`). `features_match` keeps `print and (min-width: 1px)`
  on (the old test applied it whole) and never under `not`. Side effect, toward Chrome: `all and (orientation: portrait)` and `not screen` now apply (the substring test dropped them).
- **F87e, `<template>` / `<noscript>`: no skip (corrected after the first version of 7.2).** The first version of the fork skipped `<style>` inside both, as Chrome-with-JS does. Correction from the coordinator: there is
  no scripting here, so `<noscript>` content applies, and the break reader must agree with our own engine's cascade, not with Chrome with JS. The agreement test
  (`column_reader_agrees_with_stylo_on_template_and_noscript_styles`, fork `a5977c9`) showed Stylo applies a `<style>` in `<noscript>` (agreement, as the correction said) **and in `<template>`** (Blitz does not make
  template contents inert; the correction assumed it did). The skip for `<template>` therefore contradicted Stylo and made one sheet apply half its declarations, so the harvester skips neither, for layered and unlayered
  rules. The agreement test was red with the `<template>` skip in place and is green without. The harness cases `template`, `template_unl`, `noscript` are intended differences from Chrome-with-JS (the first two are
  Blitz's `<template>` bug, F87e); ours pins them in `styles_inside_template_and_noscript_follow_the_engine`.

**Red before** (against `3b158b6`): fork lib `avoid_and_column_values_map_to_no_forced_break`, `a_later_avoid_value_overrides_an_earlier_page`, `always_is_a_value_of_the_legacy_properties_only`,
`extract_column_style_table_evaluates_the_media_attribute`, `column_reader_agrees_with_stylo_on_template_and_noscript_styles` (red with the `<template>` skip); ours `tailwind_breaks.rs` `a_later_avoid_value_cancels_an_earlier_forced_break`,
`always_breaks_only_as_a_legacy_page_break_value`, `style_media_attribute_is_a_media_query_list_for_print`, `styles_inside_template_and_noscript_follow_the_engine` (the last pins the engine-following behaviour, red only against the first version's skip). **Green after:** all.

**Harness** (`c1.py`-`c12.py`, Chrome / main / branch): **0 regressions**, except that `template` and `noscript` (c7) differ from Chrome-with-JS by design, see F87e. Mismatches left, all the same as main: 28 in `c2` (media features and responsive variants: F87a), `tw_bf_page_sm` and
`tw_bf_page_nested_amp` (F87a), `cdc_in_middle`, `sel_list_bad` (selector grammar). `template_unl` stays (Blitz applies `<template>` styles, F87e); `c8` mismatch 1 -> same as main. `fuzz.py 2 60`: 0 bad.

| Check | Result |
|---|---|
| fork `--lib` | crates.io Blitz 2073 passed, the same 13 failed; local Blitz 2086 passed, 0 failed |
| fork `render_smoke` | crates.io Blitz 250 passed + the same 3 failed; local Blitz: all 42 `tests/*.rs` one by one: green |
| fork `fulgur-vrt`, 21 examples | green, no golden changed; examples byte-identical |
| fork fmt, `clippy -p fulgur --all-targets -D warnings` | clean |
| ours `cargo test --release --locked` | 347 passed, 0 failed (22 in `tailwind_breaks.rs`); `cargo fmt --check`, `clippy --release --locked --all-targets -D warnings` clean |
| fixtures 01-13 | `cmp` byte-identical to main's binary (13 of 13); `compare.py` numbers unchanged |

Not verified: `break-before: column` inside a real multicol (the table has no column breaks, so it is "none"); `<noscript>` with scripting disabled (Chrome's print has it on).
