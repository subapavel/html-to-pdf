# F107: an `<img>` in a running element is not painted in the margin box

Date: 2026-11-04. Finding: F107 (`2026-10-15-m0-findings.md:820-822`), pilot bar item. Status: done (steps 1-4; `vrt-validator` and `pdf-reviewer` are run by the coordinator).

- Fix on the shared fulgur fork `subapavel/fulgur` `htmltopdf/srgb-colours`, as one commit on top of its head at the time (the fork is linear; rebase before pushing if another finding landed first).
- Line references are to `/home/user/fulgur` as read today; the commit was not confirmed with git. No `css-spec-researcher` output; every spec reference is from memory and **unverified**.

## 1. Problem

A logo in a running header, the GCPM way of putting a letterhead on every page, is not painted:

    <style>
      .hlavicka { position: running(hlavicka); }
      @page { size: A4; margin: 15mm; @top-left { content: element(hlavicka) } }
    </style>
    <header class="hlavicka flex items-center gap-3 text-sm">
      <img src="logo.png" style="height:32px" alt=""> <span>Příkladná integrace s.r.o.</span>
    </header>

`--image logo.png=…` is bundled, the text appears in the band on every page, the image on none (`pdfimages -list`: no image). The same happens for `src="logo.svg"`. Today's evidence: `crates/htmltopdf/tests/running_elements.rs` R4 (`:583-588`) accepts 0 or 1 image per page because it gets 0. No fixture shows it: fixture 17's footer has no image.

## 2. Spec (unverified)

- **CSS GCPM 3 §2.2** (running elements): `position: running(name)` takes the element out of the flow; `element(name)` in a margin box's `content` places it there, laid out as that box's content. An `<img>` in it is ordinary content of the margin box.
- **css-page-3 §4** (page-margin boxes) and **§5.3 / §5.3.2** (sizing): the horizontal boxes of an edge are sized from the max-content / min-content widths of their contents, so the logo's width is part of what `@top-left` measures. Default `vertical-align: middle` for top boxes (F103).
- **CSS 2.1 §10.3.2 / §10.6.2, css-images-3 §5**: a replaced element with `auto` width and height (or one of them) uses its natural size and ratio. This is the rule F9 implemented for the body.
- **What Chrome does:** Chrome does not implement `running()` / `element()`. It prints the header in the flow and leaves the margin box empty. The reference has to come from the spec arithmetic plus a hand-built Chrome page (§4.2).

## 3. Where it belongs, and why the image is dropped

In the fulgur fork. Blitz sizes an `<img>` correctly once it gets a `Resource` event; fulgur's net provider never serves the bundle. That is why F9 put the seeding in fulgur. `htmltopdf` cannot reach the margin documents. Upstream fulgur 0.40.0 most likely has the same gap (not checked); we do not report it (standing decision).

Hypotheses, ranked:

1. **Not registered (certain, read).** Stage 3 of `MarginBoxRenderer::render_page` builds its `ConvertContext` with `assets: None` and `svg_checks: None` (`render.rs:5795-5809`). `convert_image` returns `false` at `let Some(bundle) = assets` (`convert/replaced.rs:203`), so no `ImageEntry` or `SvgEntry` is made. This holds for PNG and SVG alike.
2. **Natural size not seeded (certain, read; effect to be measured).** `parse_and_layout_margin_doc` (`blitz_adapter.rs:303-317`) goes from parse to `resolve` without `seed_image_natural_sizes` (called only at `engine.rs:239`, `:1015`, `:1100`). An `<img>` not sized on both axes therefore lays out 0 wide, in the stage 1a measure document as well as in stage 3. An SVG `<img>` has no `image_data`, so `extract_inline_svg_tree` fails and it is never drawn, with or without assets (`replaced.rs:222-228`). R4 hides this because its PNG has `width` and `height`.
3. **Drawable kind not handled (unlikely).** Stage 3 paints through `draw_v2_page`, the body dispatcher. An inline `<img>` goes through `convert_inline_box_node`, the same path as the body (`inline_root.rs:596-653`). A flex-item or block `<img>` (Tailwind preflight) needs a page-0 fragment from `run_pass_with_break_styles`. Check this only once 1 and 2 are fixed.
4. **`src` lost in the copy (ruled out).** `gcpm/running.rs:128-134` serialises every attribute.

Measure first, as red tests (Step 1):
- (a) R4's sized PNG with only the assets passed through. Expect 1/1, which confirms H1 alone.
- (b) an unsized PNG. Expect 0 until H2 is fixed.
- (c) an SVG `<img>`. Expect 0 until H2 is fixed.
- If (a) is still 0 after H1, look at H3.

The change:

    // blitz_adapter.rs
    impl MarginDocEnv {
        pub(crate) fn new(margin_css: &str, font_data: &[Arc<Vec<u8>>], system_fonts: bool,
                          generics: &BTreeMap<String, String>, assets: Option<Arc<AssetBundle>>) -> Self;
        pub(crate) fn assets(&self) -> Option<&AssetBundle>;
        pub(crate) fn svg_checks(&self) -> &Arc<SvgImageChecks>;
    }
    // render.rs (new parameter after `generics`)
    pub fn render_v2(/* … */, assets: Option<&Arc<AssetBundle>>, /* … */) -> Result<(Vec<u8>, Vec<DroppedText>)>;
    impl<'a> MarginBoxRenderer<'a> { pub(crate) fn new(/* … */, assets: Option<&'a Arc<AssetBundle>>) -> Self; }

- `parse_and_layout_margin_doc` keeps its signature and calls `seed_image_natural_sizes(&mut doc, assets, Some(env.svg_checks()))` between `env.parse` and `resolve`.
  - The seed has to be in all three stages (1a, 1b, 3). Without it the max-content width of `@top-left` leaves the logo out (§5.3.2), the rectangle is too narrow, and the name wraps or overflows in stage 3.
  - The seed has to use the environment's checks, not `None`. `offline_options()` has an empty font database (`svg_images.rs:146-161`), so text in a margin-box SVG logo would not be drawn (F152).
- Stage 3's `ConvertContext` gets `assets: env.assets()` and `svg_checks: Some(Arc::clone(env.svg_checks()))`, so the SVG backgrounds that become reachable get the bundled fonts too.
- `engine.rs:860` passes `self.assets.as_ref()`.

## 4. Acceptance

**Fork red tests** (both are needed: the fork's coverage job does not see VRT):
- `render.rs` tests, next to `margin_doc_dump` (`:9033`, which gets the assets too): `margin_box_img_is_seeded_and_converted`. A bundled unsized PNG and an SVG in a running element get an `ImageEntry` / `SvgEntry` with the natural size (the PNG scaled to `height: 32px`).
- `tests/render_smoke.rs`: `running_header_with_bundled_png_and_svg`.

**`crates/htmltopdf/tests/running_images.rs`** (new). Pixels via `pdftoppm` at 96 dpi as in `inline_img_strut.rs`, with a solid red PNG and a blue SVG; words via `pdftotext -bbox`. Two-page documents.
- **I1:** R4 tightened to `images_per_page(&x, 2) == [1, 1]` (`running_elements.rs:586-587`, `:605-609`).
- **I2:** an unsized PNG at `height: 32px` in `@top-left`.
  - Red rows on both pages at (56.69 − 32) / 2 ≈ 12.3 px from the top, ±1 px.
  - x from 56.7 px; width 32 × the natural ratio, ±1 px.
- **I3:** an SVG `<img>` (`width="96" height="32"`, `<text font-family="Inter">ISO 9001</text>`) in `@top-right`.
  - Blue pixels in the band on both pages.
  - `pdftotext` finds `ISO` in the band on both pages; `pdffonts` lists only bundled Inter.
  - With `strict_text` there is no `SvgFontFallback`.
- **I4:** logo plus name. The name is on one line and starts at or right of the logo's right edge plus the gap (shows that the measure stage counts the image).
- **I5 (guard):** an unbundled `src` paints nothing and does not panic, as for a body `<img>`.
- **I6:** determinism. Two renders, and four threads (`determinism_threads.rs` pattern), give byte-identical PDFs. Record whether both pages share one image object (`pdfimages -list` object ids). If they do not, that is a low finding on size, not a blocker.
- **I7:** words in the content area equal the reference with the header deleted (the `running_elements.rs` check (a)).

**Fixture `47-running-header-logo`** (46 is taken by F217, PR #95). There is none with an image in a margin box today (only 17 uses `running()`).
- Content: 17's two-page invoice body; `logo.png` (fixture 03's, as 27 did) plus the company name via `running(hlavicka)` in `@top-left`; a bundled `znacka.svg` with SVG text via `running(znacka)` in `@top-right`.
- Constraints:
  - Rules in an inline `<style>` with simple selectors (F105).
  - No `:first` (F140) and no `hidden` (F104).
  - Logo under the 56.7 px band (F139).
  - Only classes already in the compiled sheet.
- What the PNG must show: on both pages, the logo and name top left and the mark top right, vertically centred in the band at identical positions; the body is unchanged.
- Script change: `scripts/render-fixtures.sh:22` and `crates/htmltopdf/tests/fixtures.rs:63` also bundle `*.svg`. Without it the SVG logo would be missing for a reason that has nothing to do with F107. No fixture references an `.svg` file today, so nothing else moves.
- **Reference.** Fixture 17's way is a `reference.html` that `render.mjs` prefers. Probe Chrome in this order and take the first that works:
  - P1: `@top-left { content: url(logo.png) " Příkladná integrace s.r.o." }` (whether Chrome paints `url()` in margin boxes is unverified);
  - P2: a `position: fixed; top: -15mm` header, which Chrome repeats on every printed page (unverified whether it paints outside the page area);
  - P3: the body with the header deleted, plus a one-page Chrome render of the header as a normal block, which pins the logo size and the gap.
- Record the choice and what it cannot show in the fixture README.

**Byte gate.** All 45 fixtures and `bench/corpus` stay byte-identical, because none has an image in a margin box. The fork's VRT goldens and the `examples/header-footer*` outputs do not change (no `<img>`). The new fixture's threshold in `compare.py` is its measured worst page + 1 point. Speed: `scripts/bench.py --check` passes.

## 5. Steps (each ≤ 1 day)

1. **Red tests:** the fork lib test and smoke case; `running_images.rs` I1-I7; R4 tightened. Run them once with only H1 fixed in a scratch build, to confirm the ranking in §3.
2. **Fork commit:** the `MarginDocEnv` assets and seeding, stage 3's context, and threading the assets through `render_v2`. All red tests go green; `cargo test -p fulgur` passes; `cargo fmt` and `clippy` are clean.
   - Speed probe: a 200-page document whose footer holds the SVG logo and `counter(page)`, so every page misses the cache: two SVG parses per page.
   - If the margin stage grows by more than 2 %, cache the seed per `src` in `MarginDocEnv` (a `RefCell<BTreeMap<String, Seed>>`; `usvg::Tree` clones cheaply).
3. **Fixture:** the Chrome probes P1-P3, fixture 47, `reference.html` and the reference PNGs, golden, threshold, README, and the `*.svg` bundling.
4. **Records:** an `upstream.md` row and the fork-patch text; F107 marked done in the findings; R4's comment updated; the margin-box half of F177c re-worded (see §6). Then `vrt-validator`, and `pdf-reviewer` (images in the writer path).

## 6. Risks and non-goals

- **Side effect, named:** with assets in stage 3, `background-image`, `list-style-image` and `content: url()` inside margin boxes start to paint. This is content that was missing and is now rendered (merge policy item 3). The byte gate shows that no fixture has any.
- **Rebase conflicts:** `render_page` stage 3 and `MarginDocEnv` are also touched by F99b and F103 and by open F139-F141 / F177c. Keep the change to those two places and land it as one commit.
- **Measure risk:** preflight's `max-width: 100%` inside the measure document's `inline-block` is a cyclic percentage. If Blitz resolves it to 0, the logo is measured 0 wide; I4 catches that.
- **Weak reference:** if P1 and P2 both fail, the comparison cannot show the header; the pixel tests carry it.
- **Non-goals:**
  - strict-mode refusal of margin-box image findings, and `<svg><image href>` lookups in the bundle from margin boxes (F177c: the environment's checks keep `images: None`);
  - inheritance of the copy from the element's original place (GCPM §2.2, F102 note);
  - F104, F105, F106, F108, F139, F140, F141.
  - `@top-left { content: url(…) }` without a running element: if the probe shows it broken, file a new finding.
- **Size:** small. The fork change is about 40 lines plus tests; three or four days in all with the fixture.

## 7. Results (2026-11-04)

Fork `subapavel/fulgur` `htmltopdf/srgb-colours`: **`5215c775`** (one commit on `09d4d25a`). Ours: branch `f107-image-in-running-element`.

**Ranking of §3, measured (step 1, red tests first, then one fix at a time in a scratch build).**

| State | Result |
|---|---|
| nothing (assets threaded through, not used) | I1 (sized PNG), I2, I3, I4, R4 red: no image on any page |
| H1 only (assets in stage 3's `ConvertContext`) | I1 green (sized PNG painted 1 / 1) and I5, I6 green; I2 (unsized PNG), I3 (SVG), I4 (name starts at 72.7 px, left of the logo) red; the lib test's max-content width is still the page width (the 0-wide image falls back to `body`, `get_body_child_dimension`) |
| H1 + H2 (seed in `parse_and_layout_margin_doc`) | all green |

H3 (drawable kind) did not matter: a block-level `<img>` (Tailwind preflight) in a flex item went through the same dispatcher. H4 stays ruled out.

**Deviations from the plan.**
- `images_per_page` counted `pdfimages -list` rows, so a PNG with alpha (fixture 03's logo) was 2 rows (image + `smask`); it counts `image` rows only now. R4 is `[1, 1]`, as planned.
- Stage 3's conversion moved into `margin_doc_drawables` (render.rs), so the lib test runs the production path instead of a copy of it. `margin_doc_dump` is unchanged.
- Fixture 47 wraps the SVG in `flex justify-end`. Tailwind's preflight makes every `<img>` a block, `text-align: right` does not move a block image, and css-page-3 §5.3 gives each of the two boxes a share of the free width, so the unwrapped mark stood at the left of a wide `@top-right` box (600-664 px instead of 641-736). That is the spec's behaviour, not a defect; I3 uses the same wrapper.
- `gap-3` is not in the compiled sheet; the header uses `gap-4` (16 px).
- No speed cache was needed (below). No golden directory: fixtures have none (17 has none either); the reference is `reference/chrome*.png`.

**Chrome probes (fixture README).** P1 (`content: url()`) paints, but at natural size (the 240 x 72 logo overlaps the heading); P2 (`position: fixed; top: -15mm`) paints nothing in the margin; P3 pins the sizes. **P1b** (the images as `background` of the margin boxes, `padding-left` for the logo) is the reference: mark 642-738 x 12-44 on both pages against ours 641-736 x 12-44; logo ink 1 px lower, name ink 1 px lower.

**Fixture 47**: compare 5.3 / 4.2 % (worst 5.3 %), 2 pages like Chrome, threshold 6.3 %. Main's binary paints neither image (`pdfimages -list` empty). Both pages share one image object (id 25); no size finding.

**Gates.**

| Gate | Result |
|---|---|
| Fork `cargo fmt --check`, `clippy --all-targets -D warnings` (Blitz `3cea708f`, Taffy `7bbedd5b`, Parley `1f23226a` patched in by `--config`) | pass |
| Fork `cargo test -p fulgur --no-fail-fast` (`CARGO_PROFILE_{DEV,TEST}_DEBUG=0`, pinned fontconfig) | 45 suites, 3138 passed, 0 failed, 2 ignored (09d4d25a: 3135; the 3 new tests) |
| Fork `fulgur-vrt` | 35 of 64 differ from their goldens as before (this container); the 35 `.actual.pdf` sha256 sums are identical to a run on `09d4d25a`: no golden moves |
| Ours `cargo fmt --check`, `clippy --release --locked --all-targets -D warnings` | pass |
| Ours `cargo test --release --locked --no-fail-fast` | 90 suites, 1087 passed, 0 failed, 14 ignored (main: 89 / 1079; `running_images.rs` is the new suite, 8 tests) |
| Byte gate, fixtures 01-45 + 7 corpus + 47, 1 and 4 rayon threads, against `htmltopdf-main-f198jf` | 52 identical; only 47 differs; 1 thread equals 4 |
| `render-fixtures.sh`, `compare.py --threshold-check` | passes; 47: 5.3 / 4.2 %; PNGs of 47 p1 and p2 looked at |
| `bench.py --check` | ok; b200-text 833 ms, b200-mixed 844 ms; b200-items 1452 ms (the 1250 ms table exception is not met on this host, and the corpus PDFs are byte-identical to main, so host noise as in F198j-f) |
| Speed probe: 200 pages, footer = running SVG + `counter(page)`, every page a cache miss | user CPU, 4 alternating runs each: main 1.69 s, ours 1.68 s (noise +-0.1 s); wall 1.0-1.6 s on a loaded host. No growth beyond noise, so no per-`src` seed cache |

**Not verified.** `vrt-validator` and `pdf-reviewer` (coordinator). Chrome cannot render a running element, so the reference shows positions, not the running-element mechanism. The strict-mode refusal and `<svg><image href>` lookups from margin boxes stay F177c (re-worded). `background-image`, `list-style-image` and `content: url()` inside margin boxes now paint (named in §6); no fixture or corpus document has one, which the byte gate confirms. `bench.py --check` ran while the host load average was about 4.

**New edge cases.** Low, synthetic: a margin-box `<img>` that is a Tailwind block sits at the left of a wide box in `@top-right` (spec behaviour, README of 47). Nothing invoice-like found.
