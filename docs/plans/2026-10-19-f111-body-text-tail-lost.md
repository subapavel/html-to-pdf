# F111: text directly in `<body>` that is taller than a page loses its tail

Date: 2026-10-19. Finding: F111 (`2026-10-15-m0-findings.md:650-652`). Status: planned. The engine fix goes on the fulgur fork `subapavel/fulgur` `htmltopdf/srgb-colours` (after `2103e35`). Our crate gets tests, the lock bump and fixture 19. The Blitz fork stays unchanged. Line references are to `/home/user/fulgur`, `/home/user/blitz` (`htmltopdf/blitz-dom-0.2.4`) and this repo as I read them; I could not run git to confirm the fork commit.

## 1. Problem

Measured on main (`33b0519`, A4, 15 mm): `<body>Line001<br>…Line200</body>` prints 1 page with 55 of the 200 lines and no warning. The same lines in `<div>` or `<p>` print 4 pages and all 200. Root cause, from reading:

1. **Blitz makes `<body>` itself the inline root.** A block container whose in-flow children are all inline-level becomes the inline formatting context root (`IS_INLINE_ROOT`, `construct.rs:278-294`). It gets no anonymous block. Its `layout_children` only hold embedded inline boxes (`:293`). `display:none` children, including running elements, do not count against "all inline" (`:261`).
2. **The paginator only splits body's children, never body.** `fragment_pagination_root` (`pagination_layout.rs:1254`) pushes one body fragment on page 0 with the full body height (`:1286-1296`) and then walks body's children (`:1319-1332`). Here those are text nodes and `<br>`s with zero layout height. Text nodes are skipped and `<br>`s get zero-height fragments (`:1446-1522`). The line path (`:1682-1755`) and the recursion gate (`:1790-1791`, "an inline root with text children only never passes the gate") are therefore never reached for body's own lines.
3. **So everything lands on page 0.** The renderer paints all of body's lines from that fragment, and `implied_page_count` (`:6802`) sees page 0 only. Lines below the page box are clipped.
4. **Why `<div>` works.** There the div is a *child* of body with lines, so the body-level line path splits it (`:1682`). Inside a wrapper, F30 splits it (`nested_line_split` `:5037`, `fragment_nested_inline_root` `:5111`).
5. **Why the warning is silent.** The F30 warning "placed whole and ends N px below the content box" (`warn_unprinted_inline_lines` `:5312`) has a single caller, inside `fragment_block_subtree` (`:4140`). Body is never a child there.
6. **Second hazard, same shape, from reading.** If body has a background, border, padding or shadow (`needs_block_wrapper`, `draw_primitives.rs:1048-1076`), convert also gives the body inline root a `block_styles` entry (`convert/inline_root.rs:131-143`). `dispatch_page_node` then returns early for body (`render.rs:856-862`, whose comment says "Body is never a paragraph … root"). If that is right, `<body style="padding:24px">Dobrý den</body>` prints **no text at all**, even when it is short.

Expected shapes (step 0 measures each one):

| # | Shape | Expected on main |
|---|---|---|
| S1 | `<body>` + 200 `<br>` lines | **fail** (55/200) |
| S2 | the same inside `<span>` / `<font>` / `<b>` (email templates) | **fail** |
| S3 | no `<body>` tag: `<html>Line…</html>`, or a bare string (html5ever synthesises body; `:7061`) | **fail** |
| S4 | 4000 words of prose directly in body, no `<br>` | **fail** |
| S5 | S1 + `body{padding:20px}` | **fail, possibly 0 lines** (item 6) |
| S6 | S1 + `body{background:#fafafa}` | **fail, possibly 0 lines**; a short body too |
| S7 | text, `<p>x</p>`, text in body (anonymous blocks, `construct.rs:309-316`, `create_anonymous_block` `:558`; `element_data()` covers them, `node.rs:513-519`) | pass (guard) |
| S8 | direct text in `<td>`, `<li>`, a flex item, an abspos box, a nested `<div>`; `body{display:flex;flex-direction:column}` + text | pass (F30/F30b/F30g); the flex body is unmeasured |
| S9 | S1 with a running footer before the text (`position: running()`, F102) | fail today; must keep the footer after the fix |
| S10 | short bare body (1 page) | pass (guard, must stay byte-identical) |

## 2. Spec

There is no `css-spec-researcher` output. Everything below is from memory and **unverified**.

- **CSS 2.1 §9.2.1.1, anonymous block boxes.** Inline runs are wrapped in anonymous block boxes only when a block container *mixes* block-level and inline-level children (S7). A container with only inline-level content establishes an inline formatting context itself (§9.2.1, §9.4.2), so Blitz's tree for S1 is correct and the fix belongs in the fragmenter.
- **CSS 2.1 §13.3.3 and css-break-3 §4.1.** The space between line boxes of a block container is a class B break point. Body is such a container.
- **css-break-3 §4.4.** `orphans` / `widows` default to 2 and are relaxed when no break satisfies them (the F30 rule, `ORPHANS`/`WIDOWS` `:4641-4644`).
- **css-break-3 §2.** Fragmentation must not lose content. css-page-3 says content outside the page box is clipped (cited for the F66g warning, `render.rs:371-372`).
- **CSS 2.1 §14.2 and §11.1.1.** Body's background and `overflow` propagate to the canvas / viewport when `<html>` does not set them. This is relevant to S6 and to the body-clip risk in §6.

## 3. Where it belongs

Options considered:

| Option | What | Decision |
|---|---|---|
| A | Blitz fork: always wrap body's inline content in an anonymous block | Rejected: not spec (§2), it changes the box tree of every such body (inheritance, `:first-line`), and it is a fork of a fork for a paginator gap |
| B | Ours: wrap bare body content in `<div>` before rendering | Rejected: rewrites the author's DOM (selectors such as `body > span` and `:first-child` change) and hides the engine bug from fulgur users |
| **C** | Fork, fragmenter: treat body as the split candidate when it is the inline root, using the existing F30 splitter | **Chosen** |

### Fork changes

**Pagination** (`pagination_layout.rs`, `fragment_pagination_root` `:1254`). Before the body fragment is pushed:

- `fn body_line_split(doc: &BaseDocument, body: &blitz_dom::Node, strips: PageStrips) -> Option<LineSplit>`
  - Returns `None` unless `body.flags.is_inline_root()`.
  - Otherwise it is `nested_line_split(doc, html, body, 0.0, 0.0, strips, 0)` (`:5037`), with `html` = `body.parent`.
  - So the barriers (`split_barrier` `:4927`: clip, multicol, transform, orthogonal) and the "taller than page 0 / first and overflowing" rule are F30's, unchanged.
- **When it returns `Some`:**
  - call `fragment_nested_inline_root(geometry, doc, body_id, body_x, body_w, 0.0, 0, strips, 0.0, 0.0, &split, 0)` (`:5111`). That gives one fragment per page, tiling `LineSlice`s, `line_split_insets` (body padding reserved on every page, as F30 does for boxes), the `MAX_PAGES` bound, and inline boxes and images on their line's page (`record_split_inline_root_descendants` `:5199`);
  - **skip** the generic body fragment and the children walk. The walk would place inline boxes a second time at their raw `location.y`. It still has to record running-element placeholders (`:1406-1426`) at page 0, because otherwise S9's footer disappears from the margin boxes;
  - body's first fragment stays on page 0, so body's `counter-reset` and `string-set` keep firing there (`:1270-1285`).
- **When it returns `None` and body is an inline root:**
  - call `warn_unprinted_inline_lines(doc, html, body, 0.0, strips.bottom(0))` (`:5312`), so the barrier cases are not silent;
  - otherwise today's path runs byte for byte (S10 and every fixture).

**Render** (`render.rs:856-862`, only if step 0 confirms item 6):

- When body has a `paragraphs` entry, skip only body's block paint. The pre-pass already paints it, `:318-334`. Then fall through to the per-fragment paragraph draw.
- Fix the comment at `:857`.

**Safety net** (render, warning only):

- `impl PaintExtent { pub fn note_bottom(&mut self, bottom: f32); pub fn bottom(&self) -> f32 }` (`draw_primitives.rs:684-712`).
- `draw_shaped_lines` (`paragraph.rs:784`, next to `note_right` `:798-800`) notes `y + Σ line.height` of the lines it paints. These are advance-based, so the quantised-box overlap of F78 does not count.
- After `draw_v2_page`, next to the right-edge check (`render.rs:371-378`): if `bottom() - page_size.height > PAGE_OVERFLOW_TOLERANCE_PT` (`:3388`), log `content extends {n:.1} pt past the bottom page edge on page {k} and is not printed`.
- **Why any page, not just the last.** Since F78 every line is painted in exactly one fragment, so a line past the edge of any page is printed nowhere.
- **Why no false positives:**
  - text under a clip or transform is not recorded (`enter_local`), so a clipped box does not warn (Chrome clips it too);
  - backgrounds are deliberately not recorded: the html root pre-pass paints the full document height on each page (`:293-303`);
  - margin boxes paint on their own canvas (`:386`).
- **Gate.** If step 4 finds a false positive in fixtures 01-18, the bench corpus or the test suite that cannot be removed within the step, the net is dropped and §7 says why. Each true hit becomes a finding.

### Ours

- No API change.
- Lock bump; tests in §4; fixture 19.
- The comment in `crates/htmltopdf/tests/page_from_css.rs` that explains its `<p>` wrapping points to the new test.

## 4. Acceptance

**`crates/htmltopdf/tests/body_inline_split.rs` (new).**

- Setup: A4, default `RenderOptions`, Inter, strict. Helpers `printed_words` (as `fallback_warning.rs:49`) and `pdftotext -bbox` (as `nested_paragraph_split.rs`). `body{margin:0}` unless the case says otherwise; `line-height:20px` for exact counts.
- For every case:
  - every `LineNNN` / `wNNNN` is printed exactly once, in order;
  - no word is below the content box bottom or more than 2 pt above its top;
  - the page count equals the same content wrapped in `<div>` (computed in the test).
- Cases:
  - S1, S2 (`<span>`, `<font>`), S3 (both forms), S4 (4000 words): red before.
  - S5 (padding): additionally, lines on every page start 20 px below the content top. Red.
  - S6 (background): red if step 0 confirms item 6.
  - S9: footer text on every page. Red.
  - Inline image on line 150: on the page of line 150. Red.
  - S7, S8, S10: guards, green before. S10 also checks 1 page and no warning.

**`crates/htmltopdf/tests/page_bottom_warning.rs` (new).** One test, because the logger is global.

- Must not warn:
  - S1, S4 and S5 after the fix;
  - a body-level `<p>` of 12 000 words;
  - fixture-style tables over 3 pages;
  - `@page{margin:0}` with 3 pages of `<p>`s (quantisation guard);
  - a `height:300vh` empty div with a background;
  - `overflow:hidden;height:300px` around 4000 words.
- Must warn:
  - an `inline-flex` around 4000 words (F30f-b, a known residual loss): "past the bottom page edge on page 1";
  - `body{transform:translateX(0)}` + S1: the F30 barrier message ("in a transformed box").

**Fork tests.**

- `pagination_layout` unit tests:
  - a body inline root taller than a page gets one fragment per page, its first on page 0;
  - its slices tile (`line_slices_tile` `:129`);
  - a short body keeps one full-height fragment;
  - a running placeholder is still recorded.
- `PaintExtent::note_bottom` unit test.
- `render_smoke`: S1 has 4 pages; S6 prints text. The fork's CLAUDE.md asks for lib-side tests for new draw paths.
- `fulgur-vrt` and the example goldens must stay **byte-identical**. No `.html` in the fork has text directly after `<body>` (grep, multiline), and the change is gated on an inline-root body that overflows. Any mover stops the step.
- `examples_determinism`: 10/11 as in F6 (the known SVG failure).

**Fixtures.**

- **01-18 byte-identical.** Every `<body>` starts with a block element (grep), so body is never an inline root there and the bench corpus (`bench.py:169-175`, which always starts with `HEADER`) is unchanged.
  - Gate: `cmp` against the pre-bump render, `compare.py` numbers identical, no new warning lines.
- **New `19-plain-text-invoice`.** It exists because the invariant needs a fixture for a pixel-moving change, and plain-text invoices or payment reminders from e-mail and ERP templates are this exact shape.
  - Content (Czech): about 85 lines in `<body class="font-sans text-[10pt] text-gray-900">`, `style="line-height:20px"` (avoids F81 drift). Text and `<br>`, `<b>` headings, item lines as text, the VAT recap, IBAN / VS, and terms. About 1.6 pages.
  - Only classes found by grep in `fixtures/tailwind/tailwind.css`.
  - Reference: `node scripts/reference/render.mjs 19-plain-text-invoice`, committed.
  - The PNG must show page 2 starting with the line after page 1's last, ending with the terms, and pdftotext must find every line.
  - Page count equal to Chrome's (2).
  - Threshold: worst page + 1 point. The worst page is expected near 01's 4.4 %; if it is over 15 %, look at the PNG and stop.

**Reviews.** `vrt-validator` reports no high-severity finding. `pdf-reviewer` is needed if the render change at `:856-862` lands, because text appears on pages that had none (the writer path, as in F78).

## 5. Steps (each one day or less)

0. **Measure (half a day).** On main's binary:
   - words and pages for S1-S10;
   - a scratch dump: body `is_inline_root`, `layout_children`, body's fragments;
   - confirm item 6 (S5/S6 short and long);
   - Chrome page count for S1 and fixture 19 if `render.mjs` takes an arbitrary file, otherwise say so.
   - **Stop** (postmortem, re-plan) if S1's body is not an inline root.
1. **Red tests (one day).** `body_inline_split.rs`, `page_bottom_warning.rs`, the fork unit and smoke tests. Record which are red.
2. **Fork, pagination (one day).** `body_line_split`, the split branch with running placeholders, and the barrier warning. Unit tests green; VRT and examples byte-identical.
3. **Fork, render (half a day, only if item 6 holds).** The body paragraph is painted under a body block style. S5/S6 green.
4. **Fork, safety net (one day).** `note_bottom`, the warning, and a run over fixtures 01-18, the bench corpus and both test suites. Triage every hit: a finding, or drop the net (§3 gate). Then fmt and clippy; push.
5. **Ours (one day).** Lock bump, all tests green, render fixtures, the 01-18 `cmp` gate, `bench.py --check`.
6. **Fixture 19 (one day).** HTML, README, reference, compare, look at the PNG, threshold.
7. **Docs (half a day).** Findings (F111 done, new findings from step 4), `docs/upstream.md`, this §7. Then `vrt-validator` and, per §4, `pdf-reviewer`.

## 6. Risks and non-goals

- **Body padding on every page.** F30 reserves the inset on every page (no `box-decoration-break: slice`), so a padded body indents continuation pages like a padded box. This is consistent with F30 and differs from Chrome.
- **Body clip.** `body{height:100vh;overflow:hidden}` (an "app shell") is a clip barrier in the fork, while CSS propagates body's `overflow` to the viewport (§11.1.1). It gets the warning, not a split. That is a non-goal; file it if step 0 sees it.
- **Inline anchors.** Inline `<a id>` and other zero-size inline elements in a split body get no geometry. Today they get a page-0 fragment. Every split paragraph already behaves this way (`:5227-5238`). Check internal links in step 1; if the anchor is lost, file a finding rather than widen scope.
- **Running elements placed mid-text.** These are recorded on page 0, not on the page of their source line. Non-goal.
- **The net can name pre-existing losses** (F109, F30f-b, oversized leaves). That is intended. The warning count in some PDFs grows, but the output does not change.
- **Non-goals:**
  - a multicol body;
  - html's own padding below the last line;
  - Chrome's monolithic slicing (ADR-0004);
  - block images and backgrounds in the net;
  - F30c parity of body-level widows.

## 7. Result
