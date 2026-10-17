# F32a — an oversized `<img>` vanishes; a corrupt image fails the render or is silently dropped

Date: 2026-10-17. Finding: F32 (first two bullets + the PR #23 `vrt-validator` note) in `2026-10-15-m0-findings.md`.
Status: **done** (§7). Fixes go on `subapavel/fulgur` `htmltopdf/srgb-colours` and `subapavel/blitz`
`htmltopdf/blitz-dom-0.2.4`; the strict-mode API is ours. No Parley fork was needed.

## 1. Problem

Probes: `/tmp/claude-0/vrt-f9/p/` (present 2026-10-17; generator `../mk.py`, runner `../r.sh`; Chrome renders beside them
as `*.chrome-1.png`). Step 0 copies them to `out/scratch/f32a/`.

1. **Wider than the content box → not painted.** `sz_700x300.html` (bare 700×300 PNG, default 15 mm margin, content
   680.3 px): ours leaves a gap and paints nothing; Chrome paints the whole image overflowing the content box. The cut-off
   is exactly the containing-block width: `ov_w793` (793 px on a 793.7 px page) paints, `ov_w794` does not. The gap is
   **twice** the image height (`ov_w794`: 476 px for a 238 px image), and no image XObject is written, so no draw call
   happened. From the code:
   - Blitz `layout/inline.rs:72-112,134`: the inline root's final pass breaks lines at the Taffy-given width (680), not
     the min-content width (794).
   - Parley 0.6.0 `layout/line/greedy.rs:216-234`: a box that does not fit at line start takes the emergency branch
     ("consume it and accept the overflow"); `:376-381` then commits a trailing empty line, and `:570-574` gives it the
     previous line's metrics — the doubled height.
   - The box should still be on line 0 (`try_commit_line`, `:770-787`). fulgur draws it only through the paragraph
     (`convert/inline_root.rs:546-601` → `paragraph.rs:904-911`, which needs a geometry fragment for the image node).
   - Which of Parley's line items, Blitz's stored box layout (`inline.rs:165-245`) or fulgur's lookup drops it is **not
     established** from reading; step 2 settles it.
2. **Corrupt raster → whole render fails, or is silently dropped.** Krilla 0.7.0 `Image::from_png` reads only metadata
   eagerly (`graphics/image.rs:200-209`); the pixel decode is deferred to `Document::finish()` (`:423-435` →
   `KrillaError::Image`), which fulgur maps to `Error::PdfGeneration` (`render.rs:397-399`). So `fmt_trunc2_png` (half the
   IDAT) fails the entire document ("unexpected end of file"); Chrome draws the decoded rows in a kept 120×60 box.
   Header-level corruption (`trunc.png` = 40 bytes, `garbage.png`, `empty.png`) fails `from_png` instead;
   `decode_image_for_v2` (`render.rs:2837-2847`) turns that into `None` and `render.rs:2824-2826` skips the image without
   a warning. JPEG is passed through after a header parse only (`decode_jpeg`, `:615-643`): `fmt_trunc_jpg` embeds
   truncated DCT data (poppler: blank; Chrome: kept box, nothing visible). The same three outcomes apply at
   `background.rs:823` and `paragraph.rs:844` (`ImageFormat::to_krilla_image`).
3. **Broken `<img>` (unregistered or empty `src`) → nothing, silently** (`convert/replaced.rs:199-205` returns `false`).
   Chrome shows a broken-image icon. We never fetch, so nobody learns that the logo is missing.

Latent: once (1) is fixed, `huge.png` (IHDR 20000×20000, tiny IDAT) will be painted, and Krilla allocates
`output_buffer_size()` ≈ 1.2 GB before failing. Today it is hidden only because it is wider than the page.

## 2. Spec (quoted from memory, wording unverified)

- CSS 2.1 §10.3.2 / §10.3.4 / §10.6.2: with `width`/`height: auto` a replaced element's used size is its natural size;
  `max-width` is the only clamp, and it is `none` without Tailwind's preflight.
- CSS 2.1 §9.4.2: an unbreakable inline box wider than the line box **overflows** it — one line box, never an extra
  empty one. CSS 2.1 §11.1.1 / css-overflow-3 §3: `overflow: visible` paints the overflow. css-page-3 §3.2: content
  overflowing into the page margin is painted; what falls outside the page box is lost.
- HTML §4.8.4.1: an image whose dimensions cannot be obtained, or which is "corrupted in some fatal way", is
  **broken**. §4.8.4.3: an `img` without a usable image represents its `alt` text (nothing for `alt=""`). §15.4.2: such
  an `img` with natural dimensions from attributes or CSS keeps a replaced box of those dimensions; otherwise it renders
  as its text / an empty inline; the broken-image icon is optional.
- Chrome (probes, reference only): keeps the box for `trunc2.png` and `trunc.jpg`; a ~16 px broken icon for header-only,
  garbage and 10-byte files; paints wide images overflowing.

## 3. Design

| Item | Where | Change |
|---|---|---|
| (1) | Layer found in step 2: Parley 0.6 `greedy.rs` (emergency branch + trailing line), Blitz `layout/inline.rs`, or fulgur `paragraph.rs` / `pagination_layout.rs` | One line box for an over-wide unbreakable box; the box keeps its position and is drawn. If it is Parley, a fork `subapavel/parley` is needed (the owner creates forks; standing decision, F1 postmortem). **Not** "break lines at the min-content width" (that widens every text line next to the image, test 5). |
| (2) | fulgur `image.rs` | `pub fn validate(data: &[u8]) -> Result<AssetKind, ImageError>`: a full decode with the crates Krilla uses (`png`, `zune-jpeg`, `gif`, at Krilla's lockfile versions so verdicts cannot drift); SVG via `usvg::Tree::from_data` without a fontdb. `ImageError { Unsupported, TooLarge { w, h }, Corrupt(String) }`. `MAX_IMAGE_PIXELS = 64 Mi`, checked from the header **before** decoding (no GB allocations on untrusted input; an A4 scan at 600 dpi is 35 Mpx). |
| (2) | fulgur `render.rs`, `background.rs`, `paragraph.rs` | One choke point `fn krilla_image(cache: &mut ImageCache, data: &Arc<Vec<u8>>, format: ImageFormat) -> Option<krilla::image::Image>`; `ImageCache` on `Canvas`, keyed by `Arc` pointer, lookup-only (cannot affect byte order). An image that fails validation is not drawn (`log::debug!`; ours warns). The layout box stays (F9 seeding already took the header size; Chrome keeps it for truncated data). Valid images go through `Image::from_png/jpeg/gif` as today — bytes unchanged. |
| (2)(3) | ours `crates/htmltopdf/src/lib.rs`, `fonts.rs` | Statuses computed once at `build()`: `BTreeMap<String, Result<(), String>>` from `fulgur::image::validate` (one decode per `Renderer`). `Renderer::check_images(&self, html) -> Result<()>` like `check_fonts`: `walk_html` also reports `<img src>` (entity-decoded), looked up with `AssetBundle::get_image`'s key rule; reports an unregistered or empty `src` and a referenced undecodable image. Default `log::warn!` per problem in document order; strict: `Error::Images(Vec<ImageProblem>)`. `<img>` without `src` and images inside comments / raw-text elements are ignored. |
| strict | ours + CLI | `RendererBuilder::strict_images(bool)` and `--strict-images` (separate from `strict_fonts`). `ImageProblem { src: String, kind: ImageProblemKind::{Unregistered, Undecodable(String)} }`. |

No px/pt arithmetic anywhere in this change.

## 4. Acceptance

`crates/htmltopdf/tests/image_robustness.rs` (pdftoppm at 96 dpi, solid-colour PNGs from a `solid_png` helper; image
XObjects counted with `pdfimages -list`; a blue 10 px `div` after the image is the "next content" marker):

1. `inline_img_wider_than_content_box_is_painted`: 700×100 red, default A4 15 mm; red bbox x0 57±1, 700×100 ±1. **Red.**
2. `inline_img_wider_than_content_box_takes_one_line`: blue top − red bottom ∈ [0, 8] px (today 100). **Red.**
3. `img_wider_than_page_is_painted_to_the_page_edge`: 1000×100, red from x 57 to 793. **Red.**
4. `block_img_wider_than_content_box_is_painted` (`img{display:block}`, no `max-width`): red or green per P3.
5. `text_beside_a_wide_img_wraps_at_the_content_box`: rightmost dark text pixel ≤ 738. Green guard.
6. `truncated_png_does_not_fail_the_render`: half-IDAT PNG → `Ok`, blue marker painted, 0 XObjects. **Red (Err).**
7. `truncated_png_keeps_its_layout_box`: blue top equals the intact image's ±1. **Red.**
8. `truncated_jpeg_is_not_embedded`: 0 XObjects. **Red (1 today).**
9. `header_only_garbage_and_empty_images_render_nothing`: `Ok`, nothing painted. Green guard.
10. `huge_declared_png_is_refused_without_decoding` (`style="width:100px"`): `Ok` in < 2 s, 0 XObjects. **Red.**
11. `broken_img_with_size_attributes_keeps_its_box`: `width=120 height=36` → blue top ≥ 36. Guard.
12. `strict_images_rejects_unregistered_empty_and_undecodable_src` (document order);
    `strict_images_ignores_srcless_commented_and_unreferenced_images`; non-strict renders of the same inputs are `Ok`.
    Red until step 6.
13. `render_with_a_broken_image_is_deterministic`: two renders, equal bytes.

Fork: `image::validate` unit tests (valid PNG/JPEG/GIF/SVG; truncated PNG, JPEG, GIF; 10-byte JPEG; 20000² without
allocation; garbage); `render_smoke`: truncated PNG through `<img>`, `background-image`, `content:url()` → `Ok`; a wide
inline image yields one XObject; a unit test at the layer step 2 names (one line, box positioned); example goldens
byte-identical.

Fixtures 01–10 byte-identical (`cmp` against main's renders); compare numbers unchanged. No new fixture: a Tailwind
template's preflight `max-width:100%` prevents (1), and (2)/(3) need broken input, which no realistic fixture contains.

## 5. Steps

0. Copy the probes to `out/scratch/f32a/` (recreate from `mk.py` if gone). Chrome 141 at A4, `@page{margin:15mm}`
   (ours ignores `@page` margins, F31; pass `--margin-mm`): P1 700 px inline image; P2 1000 px; P3 P1 with
   `display:block`; P4 wide image between two text lines; P5 `<img src=nope.png>` with `alt="Logo firmy"`, `alt=""`,
   `width=120 height=36`, no `alt`; P6 `huge.png` at `width:100px`; P7 `trunc2.png` / `trunc.jpg` with sizes. Record in §7.
1. Ours: tests 1–11 and 13.
2. Diagnose (1): a fork-side test dumping, for `ov_w794`, Parley's lines and items, the image node's `final_layout`,
   `geometry.get(img)`, `out.paragraphs[root]` line items. Name the dropping layer with file:line.
3. Fix (1) at that layer with its unit test. If it is Parley and `subapavel/parley` does not exist: postmortem, leave
   steps 3 and 7 open, land steps 4–6 and 8 in this PR with tests 1–4 ignored as `F32a-1`.
4. fulgur: `image::validate` + unit tests.
5. fulgur: `krilla_image` choke point, `ImageCache` on `Canvas`, the three call sites, `render_smoke`, goldens.
6. Ours: `strict_images`, `Error::Images`, statuses in `build()`, `check_images`, `walk_html` img sites; CLI
   `--strict-images`; `tests/fixtures.rs` registers each fixture dir's `*.png`/`*.jpg` (none today, so 03 would trip
   strict) and sets `strict_images(true)`; `scripts/render-fixtures.sh` passes `--strict-images`; test 12.
7. Ours, after step 3: lock bump, compare, look at the PNGs (01–10 byte-identical), `vrt-validator`.
8. Ours, for the PR landing steps 4–6: lock bump, compare (01–10 byte-identical), `pdf-reviewer` (PDF writer path),
   `docs/upstream.md`, findings (F32 → F32b–F32l), §7.

## 6. Risks and non-goals

- Every PNG/GIF/JPEG is decoded twice (`validate` once per `Renderer`, Krilla once per render); the step-5 PR states
  the cost for a 2000×1000 PNG and fixture 03's render time.
- A `validate` verdict that differs from Krilla's turns into a failure again: same crate versions, and a smoke test
  feeds every probe image through a full render.
- The fix for (1) changes the height of a paragraph that held an over-wide box (fulgur goldens may move; name them).
  Wide images now paint into the page margin and can overlap `@page` margin boxes, as in Chrome.
- Images over 64 Mpx are refused (Chrome decodes them); header-only PNGs keep their header-sized box where Chrome shows
  a broken icon. Both deliberate.
- Non-goals kept in F32: **F32b** an image taller than a page is drawn whole on the last page; **F32c** inline images
  do not grow the line box; **F32d** `object-fit` / SVG `preserveAspectRatio`; **F32e** ratio rules (both axes,
  `aspect-ratio: auto 1/1`, attributes vs preflight `height:auto`, `height:%` in an auto parent); **F32f** `src`
  matching beyond the exact key (`?v=1`, `#frag`, `..`, spaces — F32a only warns); **F32g** unsized SVG / `<canvas>`
  defaults, SVG `width="50%"`/`em`, text in SVG `<img>`; **F32h** EXIF orientation; **F32i** `float` on `<img>`;
  **F32j** abspos `left`+`right` stretch. Added: **F32k** `alt` text of a broken `<img>` (HTML §15.4.2; Blitz builds
  `img` as a childless replaced leaf); **F32l** partial rows of a truncated PNG. Not planned: the broken-image icon,
  `srcset` / `<picture>`, CSS `url()` in the strict scan.

## 7. As landed

### The layer that dropped the image (step 2)

Not Parley, not Blitz's stored box layout. A dump of `ov_w794`-like input (794 px image, 680.3 px viewport) showed
Parley placing the box on line 0 (`inlinebox id=6 x=0 y=0 w=794 h=238`), Blitz giving the `<img>` node its size and
location, and **a second, empty line** after it (`greedy.rs:216-234` commits the overflowing box, then one more
`break_next` step commits an empty trailing line with the previous line's metrics, `:376-381`, `:570-574`). The image was
dropped later: `pagination_layout.rs:813-858` (the `line_metrics.len() > 1` branch, `fragment_inline_root`) recorded the
paragraph's own fragments and **no descendants**, so `geometry.get(img)` was `None` and `paragraph.rs:904-911` (an
`InlineBox` line item is drawn only when its node has a fragment on the page) skipped it. Verified by logging
`geometry` keys at the draw site: the 120 px image has a fragment, the 700 px one has none, and
`record_descendant` is never called for it.

That path is taken by **every** paragraph of two or more lines, so the bug was wider than the plan thought: an
`<img>` or `display: inline-block` in any multi-line body-level paragraph was laid out and never painted
(`out/scratch/f32a/q1.html`, `q2.html`: a 120 x 60 image and a 50 x 20 inline-block before a three-line text
paragraph; neither painted). The wide image only reached it through Parley's phantom second line. The same code is
in upstream fulgur 0.40.0 (`8d81949`). The nested-paragraph path (F30) already recorded descendants.

Two fixes, so that both symptoms go (gap and missing image):

- **fulgur** (`02de1c0`): `fragment_inline_root` returns where each fragment's first line starts and its caller calls
  `record_split_inline_root_descendants` (the F30 helper) so inline boxes get fragments on the page of their line.
- **Blitz** (`0700454`): `layout/inline.rs` `break_lines` drives Parley's `BreakLines` instead of `break_all_lines`
  and takes back the final line with `revert()` when it has zero advance and follows a line that overflowed.
  Trade-off: this works around Parley instead of fixing it (no fork of Parley exists; agents cannot create one). It
  relies on `BreakLines::{break_next, is_done, revert}` semantics of 0.6.0 and on the artefact's signature (final
  line, advance 0, after an overflowing line). A real trailing empty line follows a line that holds the newline,
  which does not overflow, so it is kept; the cases Blitz produces (`<br>` after the box, two boxes) are in the tests.
  If Parley changes the behaviour the check stops matching and nothing is reverted (the second line returns, the image
  is still painted since the fulgur fix). Rejected: breaking at the min-content width (widens every text line next to
  the image, test 5) and clamping the box.

### Chrome probes (step 0), A4 with `@page { margin: 15mm }`, `out/scratch/f32a/p1-p7.*`

The plan's premise "Chrome paints the whole image overflowing" was wrong: `sz_700x300.html` had `@page { margin: 0 }`, so
the 700 px image fitted. With the 15 mm margin Chrome **shrinks the page contents to fit** (Blink print scaling):

| Probe | Chrome 141 | Ours after F32a |
|---|---|---|
| P1 700 x 100 inline | red x 57, 680 x 99 (scaled by 0.97; the blue marker is also 680 wide) | red x 56 (AA), 701 x 101: 700 px, over the right margin; marker 1 px below the image |
| P2 1000 x 100 | red 680 x 70 (scaled 0.68) | red 56 to the page edge (738 px painted), 100 high |
| P3 P1 with `display:block` | as P1 | as P1 |
| P4 text / 700 px image / text | red 680 x 98 | red 701 x 101, "after" line directly below (F32c: the image does not grow the line box) |
| P5 `nope.png`: alt text, `alt=""`, 120 x 36 attributes, no alt | broken-image icon + alt text, a bordered 120 x 36 box, an icon | nothing for alt (F32k), a 120 px gap for the sized one; marker 6 px higher |
| P6 `huge.png` (IHDR 20000 x 20000) at `width:100px` | nothing painted, 100 px box | nothing painted, 100 px box, render `Ok` (was `Err`) |
| P7 `trunc2.png` / `trunc.jpg` at 120 x 60 | PNG rows decoded so far (a 120 x 15 strip), JPEG nothing visible, boxes kept | nothing painted, boxes kept (F32l: partial rows) |

We follow CSS here, not Chrome: an unbreakable box overflows its line (CSS 2.1 §9.4.2) and `overflow: visible` content
is painted into the margin (css-page-3 §3.2). Chrome's shrink-to-fit is a print UA feature outside the specs; a document
that needs it should set `max-width: 100%` (Tailwind's preflight does).

### Commits

| Repo | Commit | What |
|---|---|---|
| `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4` | `0700454` | `break_lines`: no empty line after an overflowing inline box |
| `subapavel/fulgur` `htmltopdf/srgb-colours` | `02de1c0` | multi-line paragraphs record their inline boxes' geometry |
| | `4b3f0ea` | test: an image wider than the line takes one line (2 tests) |
| | `ce66eb1` | `image::validate`, `krilla_image`, `ImageCache`; `png`, `gif`, `zune-jpeg` direct dependencies (versions already locked) |
| ours | `c1f4863` | lock bump, `strict_images`, `Error::Images`, CLI `--strict-images`, tests |

### Tests

- Ours: `image_robustness.rs` 13, written first; **9 red before** (against the previous forks): tests 1, 2, 3, the added
  wrapped-image test, 6 (`Err` for both truncation variants), 7 (`Err`), 8 (1 XObject), 10 (`Err`), 13 (`Err`);
  **green guards** 4, 5, 9, 11. `image_strict.rs` 4 (test 12; red by construction, the API did not exist), 2
  `img_srcs` unit tests. Full `cargo test --release --locked` green; fmt and `clippy --release --locked --all-targets
  -D warnings` clean.
- Fork: lib 1989 -> 2000 (blitz_adapter 2: both red without the Blitz commit; pagination_layout 2: both red without the
  fulgur commit; `image::tests` 7 new), `render_smoke` 237 -> 240 (undecodable PNG through `<img>`, `background-image`,
  `content: url()`; wide inline image = 1 XObject; image in a multi-line paragraph = 1 XObject; not individually run
  against the old code), `fulgur-vrt` green with goldens byte-identical (no golden touched), `examples_determinism` 10 of
  11 (only `committed_svg_matches_rendered`, the host-font environment issue), `image_test`, `inline_box_*`,
  `list_style_image_test`, `background_test`, `pseudo_absolute_content_url`, `break_inside_avoid` green.
- Plan deviations: test 4 (`block_img_wider_...`) was already green (a block-level image is not in an inline root, so it
  never took the multi-line path) and stays a guard; test 5 is a pure guard (the image assertion I first put in it went
  to the added `inline_img_on_a_later_line_of_a_paragraph_is_painted`); the fixed-height guard for the line box in test 2
  compares with the same image where it fits instead of a bare range only.

### Numbers

- Compare 01-10 unchanged (4.4, 10.2/11.4/9.8, 5.7, 5.7, 21.6, 15.9/11.8, 7.2/7.1, 13.2/12.1, 5.1/4.9/0.7, 5.9/1.5 %);
  all ten PDFs `cmp`-identical to main's. No threshold touched.
- Time (release CLI, 8 runs): fixture 03 22-25 ms before and after. A 2000 x 1000 PNG at `width:100px`: 15 ms before,
  about 23 ms after (+8 ms): the image is now fully decoded three times in a one-shot CLI run (`validate` in
  `build()`, `validate` in the fork's `krilla_image`, Krilla's own decode), about 3-4 ms each. A `Renderer` that is
  reused pays the `build()` one once. A way to share the verdict (a fork API taking a pre-validated set) is not built.
- Strict zune-jpeg: `set_strict_mode(true)` is what notices a truncated scan, and it also rejects JPEGs with stray
  bytes between header segments, which viewers accept. Such a file is now skipped with a warning (and an error under
  `strict_images`) where it was embedded before; progressive, grayscale, CMYK, subsampled and optimised JPEGs from
  PIL pass.

### Not verified

`pdf-reviewer` and `vrt-validator` (run by the caller). The `<img>` in `@page` margin boxes (built with no assets), animated
or CMYK/16-bit PNG edge cases beyond the test images, and large valid rasters near the 64 Mpx limit (memory of the
two decodes) were not exercised. `render_smoke` tests were not run against the old fulgur.
