# F209: `visibility: hidden` on inline content is printed

Date: 2026-11-02. Finding: F209 in `2026-10-15-m0-findings.md` (medium: hidden text is printed and extractable). Fork: `subapavel/fulgur` branch `htmltopdf/srgb-colours`, head `d26eeec2` (the rev in `Cargo.lock`). Line numbers are from that checkout; re-read them before editing.

Scope decision (coordinator, 2026-11-02): the heading `/T` title is built from the visible runs' glyph text ranges (§4.2(3)). That is needed so hidden words do not leak into `/T`, and it also removes the per-run repetition of the text if step 1 confirms it. If the repetition is real, it is filed as F209a and closed by this PR, named in the PR text. It is the same line of code, not a second change.

## 1. Problem

Repros in `out/scratch/f66iv/c/` (A4, 15 mm, `font:14px/20px Inter`, Chrome m141 PDFs beside them as `*.chrome.pdf`):

| repro | Chrome | ours (main) |
|---|---|---|
| `vh_d1.html`: `<div>a <span class=t></span> w <span class=s>q</span> e</div>`, `.t::before{content:'tx';visibility:hidden}`, `.s::after{content:'0000';visibility:hidden}` | `a` `w q` `e`, with gaps where `tx` and `0000` would be | `a tx w q0000 e` (same positions: only the paint is wrong) |
| `vh_d2.html`: same at depth 2 (since F66i) | as above | as above |
| `vis_hidden_host.html`: `<span>a <span class='t vh'>hid</span> w</span>` | `a w` (gap), `AFTER` | `a txhid w` |
| `vis_hidden_outer.html`: hidden outer span | only `AFTER` | all words |
| `vis_vis_in_hidden.html`: `<span class=vh>a <span class='t vv'>vis</span> w</span>` | `txvis` alone, at its place | `a vis w` |

So the layout is already right and only the painting is wrong. A spacer pseudo-element or placeholder must keep its space and draw nothing. The same text is also in `pdftotext` output, copy and paste, and search. ADR-0009 gives the reasons why a text layer must not hold words that are not on the page: an invoice must not hide a draft marker or an old amount that indexers then read. Templates use `visibility:hidden` for:

- sign placeholders that keep numbers aligned;
- spacer pseudo-elements;
- "view online" links hidden in print;
- draft banners.

## 2. Spec (cited by the planner from memory; css-spec-researcher not run, section numbers unverified)

- **CSS 2.1 §11.2** `visibility`: `hidden`: "the generated box is invisible (fully transparent, nothing is drawn), but still affects layout. Furthermore, descendants of the element will be visible if they have `visibility: visible`." `collapse`: as `hidden` except on table rows, row groups, columns and column groups (§17.5.5).
- **css-display-3 §4** (Invisibility): `visibility` is inherited. An invisible box is not rendered and cannot be interacted with, and UAs are expected to leave it out of the accessibility tree. That is the basis for "no link annotation" and "no tagged content" below.
- **CSS 2.1 §17.5.5 / css-tables-3** (dynamic row and column effects): a collapsed row or column is removed from rendering and its space is given back. This is layout, so it is out of scope here (§7).
- **css-text-decor-3 §2.1**: a decorating box's line is drawn over the text it decorates. Whether it runs under a hidden descendant's text is to be measured in Chrome (step 1). Default in this plan: it does not, because Chrome paints decorations per text fragment.
- **PDF/UA-1 §7.1**: hidden text is neither content nor an artifact. It is not drawn at all.

## 3. Cause, traced (fulgur fork)

- `convert/style/opacity.rs:18-28` `extract_opacity_visible(node)` maps `Hidden | Collapse` to `visible = false`. That is right, but it is read only per box.
- `convert/inline_root.rs:49,122-129`: `ParagraphEntry.visible` takes the inline root's visibility, so it covers the whole paragraph.
- `convert/inline_root.rs:502-505`: each glyph run reads color, decoration and link from `brush.id`. Since `inline_span_style` in Blitz `construct.rs:1092-1099,1188-1198`, the brush id is the span or pseudo-element node, and its computed `visibility` is already the effective inherited value. The run reads no visibility from it. At 563-577 the run is pushed unconditionally. `ShapedGlyphRun` (`paragraph.rs:175-185`) has no `visible` field. Its siblings `InlineImage.visible` (213) and `InlineBoxItem.visible` (243, set at `inline_root.rs:622-626`) do. The multicol copy has the same gap: `convert/mod.rs:1150-1240` `shape_paragraph_glyph_runs`, brush at 1179-1182.
- `render.rs:4816-4818` `draw_paragraph_inner_paint` returns early on `!entry.visible`, and so do paragraph slices (`render.rs:2010-2014`). As a result, a hidden root with a `visibility:visible` descendant paints nothing.
- `paragraph.rs:999-1002` and `render.rs:1453-1461` skip a hidden inline box "with everything in it", so a visible child of a hidden inline-block is lost too.
- `render.rs:1564-1573` `extract_heading_title` concatenates `run.text`. That field is the paragraph's whole text, the same in every run (`paragraph.rs:169-174`). So the `/T` of a heading holds hidden words, and, from reading only, it repeats the text once per run (step 1 confirms with `qpdf`).
- A block with `visibility:hidden` is fine: its background and border are gated per entry (`render.rs:2684,3168`) and its block children have their own entries. `pagination_layout.rs:10347-10358` already guards that a visible `<p>` in a hidden `<div>` survives. Images, SVGs and pseudo images take their own computed visibility (`replaced.rs:53`, `pseudo.rs:28,178`), which inherits hidden. List markers are gated by `ListItemEntry.visible` (`render.rs:2676,4642`). `<img alt>` is gated (`convert/mod.rs:557-579`).

Blitz needs no change: the style data is there.

## 4. Fix

### 4.1 Where it belongs

Painting a box is engine behaviour in fulgur. The change goes on the fork branch `htmltopdf/srgb-colours`. No upstream PR or issue (standing decision of 2026-10-15) and no vendored source. Bump `Cargo.lock` and the fulgur row in `docs/upstream.md`.

### 4.2 Change

```rust
// convert/style/opacity.rs: computed visibility of the brush node; `true` when the node has no styles
pub(in crate::convert) fn node_is_visible(doc: &BaseDocument, node_id: usize) -> bool;
// paragraph.rs
pub struct ShapedGlyphRun { /* … */ pub visible: bool }
// convert/inline_root.rs: root visible, or any line item that paints (text run, image, inline box with visible == true)
pub(super) fn paragraph_paints(root_visible: bool, lines: &[ShapedLine]) -> bool;
```

1. **Keep a hidden run and flag it, do not drop it.** `extract_paragraph` and `shape_paragraph_glyph_runs` set `visible: node_is_visible(doc, brush.id)`. Every other constructor (list markers, tests) sets `true`; the compiler finds them all. Dropping the run at convert time would be wrong: `metrics_from_line` (`inline_root.rs:228`) and `recalculate_paragraph_line_boxes` (260) read text runs to place inline images. The dropped-text check (`note_covered`, 491-521) stays as it is, so hidden text never counts as text without a font.
2. **Draw** (`paragraph.rs:869`): `if !run.visible { continue }`, first thing in the `Text` arm, before `run_region` and the font lookup. The reasons: no glyphs, no link rectangle (so no annotation), no marked content, and no false `skip_run` warning. Hidden runs are also skipped in:
   - `draw_line_decorations` (540);
   - `lines_right_edge` (737);
   - `item_is_ink` (787);
   - `para_has_link_runs` (`render.rs:1523`).
3. **`extract_heading_title`** takes only visible runs, each as the text of its glyphs' `text_range`s, not `run.text`. Otherwise hidden words leak into `/T`.
4. **`ParagraphEntry.visible = paragraph_paints(root_visible, &lines)`**, at `inline_root.rs:122-129` and `list_item.rs:400-408,426-434`. Byte effects:
   - a visible root gives `true`, as before, so the bytes do not change;
   - a hidden root with nothing visible in it gives `false`, so the existing early returns (paint, slices, `heading_title_of`) keep it byte-identical;
   - a hidden root with a visible descendant paints only that descendant.

   The inside image marker (`inline_root.rs:96-109`, `list_marker.rs:81,105,150` hard-code `visible: true`) takes the `li`'s visibility, so it does not appear because a descendant is visible.
5. **Inline boxes** (`paragraph.rs:999`, `render.rs:1453-1461`): dispatch the content of a hidden inline box anyway, because every entry inside gates itself (block background and border, paragraph runs, image, SVG, table). The `ib.visible` flag then gates only the box's link rectangle. Reason: a `visibility:visible` child of a hidden inline-block must show (CSS 2.1 §11.2).

Per case:

| case | result |
|---|---|
| hidden inline text / inline pseudo text | not drawn, advance kept (1, 2) |
| `collapse` on non-table elements | same as hidden (already mapped) |
| backgrounds / borders of non-atomic spans | fulgur paints none for any span (F208 family). Whoever adds them must gate on the span's visibility |
| underline of a decorating box over hidden text | not drawn under hidden runs (2). Step 1 checks Chrome. If Chrome draws through, keep only the decoration of hidden runs and record it in §8 |
| inline-block / `img` / `svg` / pseudo image in a hidden span | hidden by inheritance. A visible descendant shows (5) |
| marker of a hidden `li`; visible `li` in hidden `ul` | hidden; shown (already right; guard tests) |
| hidden `td` / `tr` | text and cell paint hidden (already right; guard). `collapse` rows keep their space: §7 |
| hidden block with visible children | block children already shown; inline ones fixed by (4) |
| `<a>` hidden | no rectangle, so no annotation (as Chrome). A partly hidden link covers only its visible runs |
| tagged PDF / `pdftotext` | hidden text writes no glyphs and no marked content, so it is not in the text layer. A wholly hidden paragraph keeps today's path |

## 5. Acceptance

### Ours: new `crates/htmltopdf/tests/visibility_hidden.rs`

Helpers:

- `render`, `render_tagged` and `printed_words` (`pdftotext -bbox`) come from `nested_pseudos.rs`;
- `check_and_qdf` comes from `tagged_links.rs`; links are counted as `/Subtype /Link`;
- `ink_in(pdf, rect_pt)` is new: `pdftoppm -r 96 -gray` to PGM, with any pixel below 200 inside the rectangle counting as ink.

Head as in `nested_pseudos.rs`. Two oracles per case:

- (a) Chrome: words and x within 1 pt (measured in step 1);
- (b) the same body with `visibility` removed gives the same x and y within 0.01 pt for every word that is still printed, which proves the advance is kept.

1. `a_hidden_span_prints_nothing_and_keeps_its_space`: `vis_hidden_host`, `vis_hidden_outer`, and `collapse` on a span. Red.
2. `a_hidden_spacer_pseudo_keeps_its_space`: `vh_d1`, `vh_d2`; no `0000` and no `tx`. Red.
3. `a_visible_descendant_of_a_hidden_inline_is_printed`: `vis_vis_in_hidden` (`txvis`), a hidden `<div>` inline root with a visible span, a hidden `<p>` with a visible `::after`. Red.
4. `hidden_links_have_no_annotation`: a hidden `<a href>` gives 0 annotations and no text. In `<a href>Zobrazit <span class=vh>online</span></a>`, one annotation whose width is that of `Zobrazit` within 1 pt. Red.
5. `boxes_in_a_hidden_span`: an inline-block with background and border, an `<img>`, an inline `<svg>` and a `content:''` inline-block pseudo: no `ink_in` and next-word x unchanged (guard, green). A `visibility:visible` child of a hidden inline-block is printed. Red.
6. `decorations_follow_the_text`: `<u>a <span class=vh>bb</span> c</u>` gives underline ink as in Chrome (step 1). Red if Chrome skips it.
7. `tagged_hidden_text`: tagged renders of 1, 3 and 4 pass `qpdf --check` and `pdftotext` shows no hidden words. `<h2>Faktura <span class=vh>KONCEPT</span></h2>` has a `/T` without `KONCEPT`. Red.
8. Guards, green on main:
   - a hidden `li` has no marker, and a visible `li` in a hidden `ul` has one;
   - a hidden `td` / `tr` prints no text;
   - a `collapse` `tr` prints no text (no layout assertion, §7);
   - a hidden block prints no background, its visible block child does;
   - `opacity:0` text is still in the text layer (a different rule, not changed here).

### Fork

- `convert/inline_root.rs` tests: a run's `visible` follows its brush for a span, a pseudo-element, depth 2 and a visible-in-hidden case. `metrics_from_line` is unchanged with a hidden first run. `paragraph_paints` truth table.
- `paragraph.rs`: a hidden run writes no glyphs or link rectangle.
- `tests/render_smoke.rs`: an end-to-end smoke test (the coverage gotcha in fulgur `CLAUDE.md`).
- The existing `tests/opacity_visibility_test.rs` and `render_smoke.rs` caption and alt tests stay green.

### Fixture `37-hidden-placeholders`

A Czech invoice with an inline `<style>`. Reason: the compiled `tailwind.css` has no `.invisible` rule, and rebuilding it would move the bytes of every fixture. It holds:

- an items table whose amounts carry a sign placeholder `<span class=sign>−</span>`, hidden on positive rows and visible on the discount row, so the numbers align;
- `KS` padded with `.pad::before{content:'00';visibility:hidden}` so it lines up under `VS`;
- `Fakturu najdete také na portálu: <a class=screen-only href="https://faktury.example.cz/2026-0042">Zobrazit online</a>` with `.screen-only{visibility:hidden}`;
- a hidden draft box (`border:2px dashed #dc2626; background:#fef2f2`) reading `KONCEPT — neplatný doklad`, with a visible `<span>Doklad byl vystaven elektronicky a je platný bez podpisu.</span>`;
- a hidden inline-block `STORNO` stamp before the title.

`fixtures.rs` asserts:

- the printed words and their x within 1 pt of the reference;
- that `KONCEPT`, `STORNO`, `Zobrazit` and `00` are absent;
- exactly one `−`;
- no annotation.

The PNG must show aligned amounts, an empty gap where the stamp and draft box were, the note in place, and no link text. The diff before the fix is large; the threshold is the measured number plus 1 point.

### Byte gates (1 and 4 rayon threads, `out/scratch/f30fl/gate.sh`)

- **Fixtures 01-36 and the 7 `bench/corpus` documents:** byte-identical expected. Reason: none of their HTML, `tailwind.css` or the corpus contains `visibility` or `invisible` (grep), and the Blitz UA sheet only has `dialog:modal{visibility:visible}`. With no hidden node every run is visible, every `paragraph_paints` is the root's value, and every inline box is visible, so the output is the same. Any change stops the work.
- **Fork `fulgur-vrt` and examples:** no `visibility` in them. Compare `.actual.pdf` before and after; do not re-bless on this host.

## 6. Steps

1. **Measure Chrome m141** for tests 1-7 and fixture 37: word x and y, annotation rectangles, underline pixels, `collapse` row heights. Record in §8.
   - If Chrome's underline runs under hidden text, change 4.2(2) as stated in §4.
   - Confirm the `/T` repetition with `qpdf`. If it is real, file it as F209a, fixed here by 4.2(3).
2. **Red tests, fixture 37 and its reference; commit.**
3. **Fork, 4.2(1)-(3)**, with unit tests red first. Then:
   - `cargo test -p fulgur --lib` and `--test render_smoke`;
   - clippy `-D warnings` and fmt;
   - push, bump `Cargo.lock`, byte gate.
4. **Fork, 4.2(4).** Tests, byte gate.
5. **Fork, 4.2(5).** Tests, byte gate. If a fixture moves, revert this step only, file F209c, and record it in §8.
6. **Finish:**
   - run `render-fixtures.sh` and `compare.py`, and look at 37's side-by-side;
   - `vrt-validator`;
   - `pdf-reviewer`, required because the text layer, annotations and tags change (merge policy item 5);
   - `bench.py --check` (one style lookup per glyph run);
   - findings: F209 done, F209a and F209b filed;
   - `docs/upstream.md`; §8 of this plan.

## 7. Risks and out of scope

- **Empty marked content:** a paragraph whose visible root has only hidden runs writes an empty marked-content sequence (as ADR-0009's clipped-out paragraphs do); `pdf-reviewer` checks it.
- **A missing style counts as visible:** `node_is_visible` treats a node without styles as visible. The test for a hidden `div` with loose text next to a block child checks that anonymous blocks inherit.
- **Dispatching hidden inline boxes** (4.2(5)) relies on every entry gating itself. `draw_under_transform` and `draw_under_clip` are covered by test 5. If one does not gate itself, step 5 falls back.
- **Out of scope (file):**
  - F209b: `visibility: collapse` on rows and columns keeps its space (Blitz / Taffy table layout; Chrome removes the row, to be confirmed in step 1);
  - collapsed flex items (css-flexbox §4.4);
  - outline (bookmark) entries, `string-set` and running elements from hidden headings;
  - collapsed-border segments of hidden cells (`table_borders.rs:175` checks only the table);
  - row backgrounds behind hidden cells (F19 note);
  - inline backgrounds and borders (F208).

## 8. Evidence

### 8.1 Chrome m141 measurements (step 1; `out/scratch/f209/gen.py` writes the 41 HTML files, `words.py`, `annots.py`, `ink.py` read them)

A4, 15 mm, Inter 14px/20px, `AFTER` paragraph below where given. x in pt, within 1 pt of ours (Chrome's x is 0.2 pt right of ours everywhere).

| case | Chrome | main |
|---|---|---|
| hidden span `a [hid] w` / `collapse` span | `a` 42.7, `w` 69.7 | prints `hid` |
| `<span>a <span class='t vh'>hid</span> w</span>` | `a`, `w` 78.9 | `a txhid w` |
| hidden outer span with a nested pseudo | only `AFTER` | all |
| spacer pseudos (`vh_d1`, `vh_d2`) | `a` 42.7, `w` 63.7, `q` 75.3, `e` 111.2 | `tx`, `q0000` |
| `.vh > .t.vv` | `txvis` 51.6 | `a txvis w` |
| hidden `div` (inline root) with a visible span | `vis` 51.6 | nothing |
| hidden `p` with a visible `::after` | `ZZ` 98.2 (not the p's text) | nothing |
| hidden `div` with loose text and a visible `p` | `blockvis` 42.7,58.8; `AFTER` 88.8 | same (guard) |
| visible child of a hidden inline-block | `vischild` 70.5, `w` 136.5 (y 44.6) | not printed (our inline-block is 2.7 pt low, a separate gap: x only) |
| ink of a hidden inline-block / `img` / `svg` / pseudo inline-block | none | none (guard) |
| visible inline-block in a hidden span | shown | shown |
| `<u>a <span class=vh>bb</span> c</u>` underline | runs under `a ` and ` c`, **gap** under the hidden word (px 69-85) | the same gap (our underline is per run; with a visible span there is a gap too, a separate gap) |
| `<span class=vh><u>bb</u></span>` | no underline | underline drawn (red) |
| `<span class=vh>a <u class=vv>bb</u> c</span>` | underline under `bb` only | the same |
| hidden `<a>` | no annotation | 1 annotation |
| `<a>` holding only a hidden span | **one annotation** over the blank (51.8-81.0) | 1 annotation |
| `<a>Zobrazit <span vh>online</span></a>` | **two** rectangles, 42.8-87.0 and 87.0-116.2 (the whole link) | 1 over the whole |
| `<h2>Faktura <span class=vh>KONCEPT</span></h2>` | words: `Faktura` | `Faktura KONCEPT`; `/T (Faktura KONCEPTFaktura KONCEPT)` |
| `<h2>Faktura <b>č. 42</b> za rok</h2>` (no span hidden) | n/a | `/T` holds the text **three times** (three runs): F209a confirmed |
| hidden `li` / `li` visible in hidden `ul` | no text / text | same, one `•` (guard) |
| hidden `td`, `tr`, `tr collapse` | no text | no text (guard) |
| rows `first / tr height:60px / third`: hidden vs `collapse` | hidden keeps the 60 px (`third` 103.8); **collapse removes it (`third` 73.8)** | 103.6 both: F209b confirmed |
| `<col class=collapse>` | the column's cell text is gone, `shown` at 42.7 | `hidcolshown` printed: F209b |
| flex item hidden / `collapse` | `aa` 42.7, `cc` 73.8 both (the collapsed item keeps its main size here) | same, no text of the item (so no finding for flex items) |
| `opacity:0` span | text **not** in Chrome's PDF | `zero` printed: filed (F211) |
| hidden block background, visible child's background | only the child's | same (guard) |

Deviations from the plan's tests, from these numbers:

- **Links (test 4).** Chrome's annotation follows the `<a>` element's own visibility, not the runs: a hidden `<a>` has none (as planned), but a visible `<a>` keeps rectangles over hidden text inside it, `online` included. The plan's "one annotation of the visible run's width" is kept: a clickable blank is the kind of residue ADR-0009 removes, and css-display-3 says a hidden box cannot be interacted with (section number unverified). The test records Chrome's first rectangle (`Zobrazit ` with its space, 42.8-87.0) as the oracle for the one we write, and says why Chrome's second rectangle is not copied.
- **Decorations (test 6).** Chrome draws no underline under hidden text, as the plan's default. `<u>a <span class=vh>bb</span> c</u>` is already right on main (a different gap: our decoration is per run, so no span child of a `<u>` is underlined, hidden or not), so only `<span class=vh><u>bb</u></span>` is red.
- **Flex `collapse`** (plan section 7, out of scope): Chrome's collapsed item gives the same layout as a hidden one and ours matches, so it is no longer listed.
- **`-r 96` raster** replaced by `-r 72` in `ink_in`: one pixel is one pt, so the test converts no units.
- **`<img>` data URIs** draw nothing in our engine (F201), so the `img` case registers a bundled `dot.png`.
- **Fixture 37:** the sign placeholder trails the number (`30 000,00<span class=sign>-</span>`): with a leading placeholder in a right-aligned cell the digits end at the same x whether or not it is printed, so it would prove nothing.

### 8.2 To be filled by the implementer

What was built (commits), gates table (fork tests, clippy/fmt, byte gate 43 files x 2 thread counts, `fulgur-vrt` actuals, our tests, compare number for 37, bench), not verified.
