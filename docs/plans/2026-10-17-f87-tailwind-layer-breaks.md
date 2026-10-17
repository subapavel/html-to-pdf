# F87 — Tailwind break utilities inside `@layer` never reach fulgur's break / column reader

Date: 2026-10-17. Finding: F87 in `2026-10-15-m0-findings.md`. Status: **planned**. Fix goes on `subapavel/fulgur`
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
  - **F87b:** `!important` (Tailwind `!break-inside-avoid`), if step 1 confirms it is dropped.
  - Specificity.
  - Evaluating `@supports`.
  - `<link>`/`@import` sheets (fulgur-s5ro) and `@import … layer()`.
  - CSS nesting.
  - `revert-layer`.
  - F83a / F83b.
  - Fixture 12's inline `columns:2` (Stylo-owned; `columns-2` not compiled).
  - Rebuilding `tailwind.css`.

## 7. Result

(Filled in at landing.)
