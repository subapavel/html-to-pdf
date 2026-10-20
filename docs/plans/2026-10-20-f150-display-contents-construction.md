# F150 + F153: `display: contents` in Blitz layout-children construction (panic and silent content loss)

Date: 2026-10-20. Findings: F150 (`2026-10-15-m0-findings.md:832-834`) and **F153** (new, severity high: author text is dropped silently). Status: planned. The engine fix goes on the Blitz fork only (`subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`). The fulgur fork gets one test-only commit (§6). Our crate gets tests, the lock bump and fixture 21. Line numbers refer to the working tree of `/home/user/blitz` on that branch, which should be `d5d60b7`. I could not run git to confirm this. The investigation's "~258-270" and "~453-550" are `construct.rs:232-244` and `:463-554` here.

## 1. Problem

Blitz builds `layout_children` (the box tree that Taffy and fulgur walk) in `collect_layout_children` (`construct.rs:127`). For a `display: contents` child, `collect_complex_layout_children` calls `collect_layout_children(doc, child_id, …)` (`:514-516`). That call enters the `Contents` arm (`:232-244`), which runs `collect_layout_children` again on every grandchild. That computes the grandchild's own layout children into the parent's list. It never pushes the grandchild itself. Two bugs follow:

- **F153, silent loss.** A child of the contents element that has only inline content becomes an inline root but is never placed. A `<p>` loses its text. Wrapping the child in a `<div>` "works" only because the `<div>` box is dropped and its children get pushed instead (`/tmp/claude-0/f150/repro/control-no-loss.html`). Verified on main:
  - `<body><div style="display:contents"><p>Faktura</p><p>Celkem</p></div><p>end</p></body>` prints only `end`.
  - `<p>start</p>` followed by the same wrapper prints only `start`.
  - `<div><div style="display:contents"><span>Faktura</span> <b>Celkem</b></div><p>y</p></div>` prints only `y`.
  - Related, from reading: when a contents element is the only child, the parent's "all inline" check skips it (`:258-259`). So `<body><div style="display:contents"><p>…</p><p>…</p></div></body>` makes body an inline root, and the `<p>`s become inline boxes side by side (`:762-764`). fulgur already carries a workaround and a test for exactly this (`pagination_layout.rs:1462-1469`, `:12105`).
- **F150, panic.** The trigger is mixed content, then a whitespace-only text node (which opens an anonymous block, pushed at `:519-524`), then a contents child whose recursion pushes ids after that block. The cleanup at `:533-538` / `:547-552` runs `layout_children.pop(); doc.nodes.remove(anon_id)` on the assumption that the anonymous block is last. It pops the wrong id and frees the anonymous block while its id stays in the list. `resolve_layout_children_recursive` then hits `document.rs:1146` "invalid key". Repros are in `/tmp/claude-0/f150/repro/` (`f150-a/b/c`, `notes.md`). The diagnosis comes from reading, without an instrumented build; step 0 confirms it.

## 2. Spec

There is no `css-spec-researcher` output. The spec items are from memory and **unverified**. The Stylo items are verified in source.

- **css-display-3 §2.5, `contents`.** The element generates no boxes. Its children and its `::before`/`::after` generate boxes as normal. For box generation and layout it is treated as if it were replaced by its contents. Inheritance and selector matching follow the element tree and are unaffected.
- **css-display-3 §2.7.** `contents` on the root computes to `block`. Verified: Stylo `values/specified/box.rs:336-340`. "Unboxing" of replaced elements and form controls is gecko-only in Stylo (`style_adjuster.rs:545-568`), so Blitz does not apply it.
- **Stylo layout parent.** The layout parent skips contents ancestors (`style_resolver.rs:131-138`, `matching.rs:914`), so flex and grid items under a contents wrapper are already blockified. Verified.
- **CSS 2.1 §9.2.1.1.** Maximal runs of inline-level content in a block container with block-level siblings are wrapped in anonymous block boxes. The runs are taken in box-tree order, so a run can cross a contents boundary.
- **css-flexbox-1 §4 / css-grid-2 §6.** Each in-flow child of the container is an item, after contents flattening. Each contiguous run of text is wrapped in one anonymous item. Whitespace-only runs are not rendered.
- **CSS 2.1 §17.2.1 / css-tables-3.** Anonymous table objects are generated after contents flattening. Already implemented by F50's `flatten_contents` (`table.rs:805-820`).
- **css-lists-3.** `list-item` numbering follows the element tree, so an `<li>` inside a contents wrapper is still numbered by its list.

## 3. Where it belongs

Box construction is engine behaviour, so the fix goes in the Blitz fork per the standing decision. Rewriting the DOM in our crate was rejected: it changes what selectors match (same reason as F111 option B).

**One PR for both findings.** They have the same cause: the `Contents` path computes layout children for the wrong node. The F153 fix places the children of a contents element through the same per-child routine as ordinary children. Then any non-wrapped push first closes the open anonymous block (and frees it if it holds only whitespace). So the open anonymous block is always last when it is closed, and F150's precondition cannot occur. Fixing F150 alone (pop by identity) would touch the same 15 lines and leave a known-wrong tree behind. Removal by identity is kept as defence in depth.

Changes in `packages/blitz-dom/src`:

- **`layout/construct.rs`**
  - The body of the closure at `:485-544` becomes `fn place_box_child(doc: &mut BaseDocument, container_node_id: usize, child_id: usize, layout_children: &mut Vec<usize>, anonymous_block_id: &mut Option<usize>, hide_whitespace: bool, needs_wrap: &dyn Fn(NodeKind, DisplayOutside) -> bool)`. For a contents child it calls `flush_pseudo_elements(child)` (`:378`), clears its construction damage (as `:233-234` did) and recurses over the child's `before`, `children` and `after` with the same container, list and anonymous block.
  - `fn close_anonymous_block(doc: &mut BaseDocument, layout_children: &mut Vec<usize>, anonymous_block_id: &mut Option<usize>)` replaces both cleanups. It removes the whitespace-only block by identity (`rposition`), with `debug_assert!` that it was last.
  - `fn flattened_box_children(doc: &mut BaseDocument, node_id: usize, with_own_pseudos: bool) -> Vec<usize>` flushes contents pseudos as it goes. `fn has_contents_child(doc: &BaseDocument, node_id: usize) -> bool` keeps the common path free of allocation.
  - The Flow arm classification (`:246-276`) iterates `flattened_box_children(.., false)` when `has_contents_child`. A contents element is never classified itself.
  - The `_` arm (`:372-374`, e.g. orphan `table-row`) and the `Contents` arm (`:232-244`) push `flattened_box_children(.., true)`. The `Contents` arm is now reached only by such defensive paths, because the root is blockified. It never drops content.
  - Inline path: the `Contents` arms at `:709-719` and `:899-911` also visit `before`/`after`.
  - In `:899-911`, push a style span built from the contents element's computed style. Factor `:935-954` into `fn inline_span_style(node: &Node, root_line_height: f32) -> parley TextStyle`.
  - In `build_inline_layout_into` (`:839-848`): when the root is an anonymous block and a child's DOM parent is a contents element, wrap that child in the DOM parent's span. Otherwise text placed from inside a contents element takes the anonymous block's style.
- **`node/node.rs:215-216` `is_or_contains_block`.** `(None, Contents)` recurses into the children.
- **`layout/list.rs:22-34` `collect_list_item_children`.** In the non-list-item branch, recurse into contents children so their `<li>`s get markers and numbers.
- **`table.rs` stays untouched.** It already flattens, and leaving it alone keeps F36 slot order and F127 as they are.

## 4. Acceptance

The new file is `crates/htmltopdf/tests/display_contents.rs`, in the style of `body_inline_split.rs`: Inter bundled, strict fonts, `pdftotext` / `-bbox`, unique tokens (`wNNNN`). `c` below means `style="display:contents"`.

| ID | Shape | Main today | Must show / assert |
|---|---|---|---|
| C1 | `<div c><p>Faktura</p><p>Celkem</p></div><p>end</p>` in body | only `end` | all words once, in order; two lines |
| C2 | `<p>start</p><div c>…two p…</div>` | only `start` | same |
| C3 | `<div><div c><span>Faktura</span> <b>Celkem</b></div><p>y</p></div>` | only `y` | `Faktura Celkem` on one line, then `y` |
| C4 | `<div>a <span c>b <i>c</i></span> d<p>e</p></div>` (run across the boundary) | measure | `a b c d` on one line |
| C5 | three nested contents, mixed text, blocks and whitespace | measure | order and lines as without wrappers |
| C6 | body whose only child is contents: (a) blocks, (b) inline only | (a) side by side | (a) stacked; (b) body stays inline root (F111 guard) |
| C7/C8 | flex row / 2-column grid with a contents wrapper around items and text | measure | items in a row / 2×2 grid; text becomes one anonymous item |
| C9 | `tbody c`, `<td><div c><p>`, `table c` with content (orphan rows) | measure | words once (geometry of the orphan case is F50b) |
| C10 | `<ol><div c><li>Prvni</li><li>Druhy</li></div><li>Treti</li></ol>` | lost | markers `1.` `2.` `3.` |
| C11 | `<p>a <span c style="font-size:30px;color:red">B</span> c</p>` | B small, black (from reading) | B glyph box about 30 px; darkest pixel in its box is red (`pdftoppm`) |
| C12 | `.c::before{content:"Pred"} .c::after{content:"Po"}` in block, mixed and inline contexts | measure | `Pred … Po` around the children |
| C13 | running header inside a contents wrapper (F102) | measure | once in the margin box, not in the flow |
| C14 | 150 `<p>LineNNN</p>` plus text runs in a wrapper, about 4 pages | lost | every line once; page count equals the unwrapped version |
| C15 | F150 a/b/c, plus a variant with newlines and indents between every tag | `EnginePanic` | `render` returns `Ok`; words preserved |
| C16 | `body{display:contents}` | measure | `Ok`; if words are lost, file a finding and `#[ignore]` with it |

**Geometry.**
- Oracle: a contents wrapper without styles must not move anything. For C1-C5, C7, C8, C10 and C14, word bboxes must equal those of the same document with the wrappers removed, within 0.01 pt.
- Chrome: C1, C4 and C7 are also checked against constants measured in step 0, within 1.5 pt. Use `/tmp/claude-0/-home-user/a456d4dd-daaf-5a66-be03-3af9ac054072/scratchpad/render-ref.mjs` with the same Inter font.

**Blitz unit tests** (`construct.rs` `#[cfg(test)] mod contents_tests`). Trees are built with `doc.mutate()` and `id` / `style` attributes, then `doc.resolve(0.0)` runs.
- `layout_children` of the named nodes equal the flattened lists for C1-C4, C7 and C10.
- Every id in every `layout_children` is a live slab key (the F150 guard).
- `resolve` twice gives the same tree.

**Fuzz test.** Yes, in our crate.
- Generator: seeded xorshift with no new dependency. Random trees of `div` / `span` / `b` / `p` (inline content only), contents wrappers, `display: flex|grid|table*` divs (no `<table>` tags, because html5ever moves a `<div>` out of them), lists and whitespace.
- Each seed renders wrapped and unwrapped. Assert `Ok`, each word exactly once, and bboxes equal.
- 64 seeds in CI, measured in step 3 and kept at 15 s or less in the debug profile. An `#[ignore]` run of 2000 seeds is available, and `DISPLAY_CONTENTS_FUZZ_SEEDS` overrides the count.
- Why it is worth it: the investigation saw about 3 % panics per whitespace tree, so 64 seeds catch an F150 regression with about 86 % probability. The fixed repros catch it every time.

**Fixtures.**
- `contents` appears nowhere in fixtures 01-20, the bench corpus, `tailwind.css`, fulgur VRT or fulgur examples (grep). So 01-20 must stay **byte-identical** (gate) and the pixel diff is unchanged.
- New `fixtures/invoices/21-display-contents/` (Czech invoice, component-style wrappers around the address blocks, the item rows of a div grid, `li`s and a `::before` label). It uses inline `style` / `<style>`, so `tailwind.css` is not rebuilt. Chrome references are committed and the threshold is set from its first number.
- One fulgur test moves: `tables_under_display_contents_are_not_counted_as_a_column` (`pagination_layout.rs:12105`). It pins the side-by-side layout of C6a.

## 5. Steps

0. **Measure (½ day).**
   - Render C1-C16 on main and in Chrome, and record the Chrome constants.
   - Confirm both causes on repro `f150-a` with a temporary `layout_children` dump.
   - Measure `text-decoration` on a contents element in both engines.
   - Write the red tests (ours and Blitz).
1. **Blitz block, mixed, flex and other paths (1 day).** `place_box_child`, `close_anonymous_block`, the flattened classification, the `_` and `Contents` arms, `is_or_contains_block`. Green: C1-C9, C14, C15, and the Blitz unit tests.
2. **Blitz inline path and lists (≤1 day).** Pseudos in both inline arms, the style spans, `list.rs`. Green: C10-C12.
3. **Wire-up (½ day).**
   - Rewrite the fulgur test to assert stacked tables: placed once, 4 pages at 400 px.
   - Bump our lock. Green: C13, C16, the fuzz test.
   - `bench.py --check` must pass, so the speed guard holds.
4. **Fixture and records (½ day).** Fixture 21 with references, `compare.py`, `vrt-validator`. Add a row to `docs/upstream.md`. Mark F150/F153 done in the findings file and file §6's new findings.

## 6. Risks and non-goals

**Risks**

- **F127 / F36.** `table.rs` is unchanged, so slot order is the same and a `thead` in a wrapper is still not repeated.
- **F144.** `SubtreeFacts` reads DOM `children` only, never layout (`subtree_facts.rs:61-62`; there is a test at `:347`). A wrapper only adds one level against the depth bound.
- **F111.** C6b guards that body stays the inline root for inline-only content.
- **F102.** The placeholder climbs DOM parents and siblings (`pagination_layout.rs:5823-5847`). A contents node has no fragment, so the climb passes through it.
- **Brush id.** Text directly in a contents element gets that element's brush. fulgur reads colour, decoration and link from the brush (`convert/mod.rs:1108-1110`). Links on `a{display:contents}` start working. A `text-decoration` on the wrapper may be drawn where Chrome does not draw it (step 0). If they differ, that is a new finding.
- **fulgur `prev_plain`** (`:1462-1469`) may lose its only trigger. Leave it and check in a follow-up.

**Non-goals** (each is filed if confirmed):

- §2.7 unboxing (`select{display:contents}` shows option text).
- `display: contents` on `::before`/`::after` themselves.
- Pseudos of contents elements inside table contexts (`flatten_contents` ignores them).
- Pseudos of inline elements two or more levels deep in an inline formatting context are never flushed: `flush_inline_pseudos_recursive` tests the parent's display (`construct.rs:669-671`). From reading; separate finding.
- F50b orphan-table geometry.
- Anonymous blocks leaked on re-resolve (existing behaviour).
- Tagging of contents elements.

## 7. Result

Implemented as planned (Blitz fork `0819dd4`, `b4cbc77`; fulgur fork `2dc4c2b` (a test) and `96ebb8a` (the review fixes below); our crate: tests, lock bump, fixture 21).

**Step 0, main `ff67bde` against Chrome** (probes C1-C16 in the order of §4, A4, Inter, positions relative to the first word; Chrome 1.5 pt is the tolerance of the test):

| Probe | Main | Chrome / fixed |
|---|---|---|
| C1, C2, C3 | only `end` / only `start` / only `y` | all words, stacked or on one line as in Chrome |
| C4, C5 | `a d` / `w1 w9 w10` (inner words lost) | `a b c d`, e on the next line; 7 lines like Chrome |
| C6a | `FakturaCelkem` side by side on one line | two lines, 15 pt apart |
| C6b | fine | fine (guard) |
| C7, C8 | `tx three` only / `loose g4` only | `one two tx three` (offsets 28.8 / 57.2 / 75.2 pt, ours 29.3 / 57.8 / 75.8); 2 x 2 grid |
| C9 a/b/c | fine (x differs by 1.7 pt in cells, table-spacing drift, not this finding) | same |
| C10 | `1.` and `Treti` only | `1.` `2.` `3.` |
| C11 | B small, black | B 30 px, red (the line placement differs from Chrome with and without a wrapper: F158) |
| C12 | pseudos and blocks lost in block and mixed context | `Pred / blk / Po`, `PredinlPo`, `m Predmix iPo` |
| C14 | 1 page, runs side by side, every line glued | 3 pages like Chrome and like the unwrapped version |
| C16 | nothing printed (`body{display:contents}`) | both words |
| F150 a/b/c | `EnginePanic: invalid key` | `Ok`, words preserved |

Causes confirmed with the red Blitz unit tests rather than a dump: `layout_children` of the body in C1 lacks the `<p>`s (F153), and the F150 trees panic at `document.rs:1146` inside `doc.resolve`. `text-decoration` on a contents element: Chrome draws none, we draw it (F156). Chrome keeps the colour and font of the wrapper for its text.

**Red / green.** Blitz `contents_tests` (new, 11 tests; blitz-dom 86 tests in all, 0 failures): on `d5d60b7` 7 of the first 8 fail (the F150 one with "invalid key"), the rest were added with the fixes. Ours `display_contents.rs` (19 tests + 1 `#[ignore]` long fuzz after the review) on Blitz `d5d60b7`: 17 fail, C6b passes (it is the F111 guard); on the fix all 18 pass. Fuzz on main: 94 failure records in the first 64 seeds (12 of them `EnginePanic`); on the fix 0 of seeds 0-1999 (92 s release, 46 ms per seed; CI runs 64 seeds, 3 s release). The fulgur test `tables_under_display_contents_are_stacked_like_tables_outside_it` passes on the fix (4 pages, 19 tables on page 1, same cells as the unwrapped document); not run against the old Blitz. fulgur `cargo test -p fulgur --lib` against the fork: 2220 passed.

**Fixture 21** (`fixtures/invoices/21-display-contents`): Chrome reference committed; 1 page, Chrome 1. Main's binary 7.4 % (the items grid, the notes, two terms and the supplier's lines are missing), the fix 3.7 %; `compare.py` threshold 0.047 (3.7 + 1 point). Fixtures 01-20 keep their numbers (the threshold check passes on all 21). I looked at `out/compare/21-display-contents-p1.png`: the page matches Chrome except the bold amount in the monospace `dd` (the bundled mono has no bold).

**Bytes.** Fixtures 01-20, the bench corpus (`b200-*`, `slow-*`) and the scaling variants against main's binary, 1 and 4 threads: 92 of 92 identical.

**Review of the branch** (vrt-validator, pdf-reviewer) found two defects of the change itself; both are fixed in the fulgur fork (`96ebb8a`), our lock bumped.

1. *Tagged PDFs.* A link that becomes several boxes put the annotation identifiers into the `/Link` group of each paragraph and krilla panicked ("AnnotationIdentifier appears twice in the tag tree", `tagging/mod.rs:1176`), newly reachable with `<a style="display:contents">` around flex items or grid cells and on main already with `<a href><p>A</p><div>B</div></a>`. `build_tag_group` now attaches the annotations of a link span to the first `/Link` group that holds it (first wins; a set of attached link spans is threaded through the walk), so every annotation has one parent (ISO 32000-1 §14.7.4.3, PDF/UA-1 7.18.5). Later paragraph groups of the same link hold their content only; the annotation rect is unchanged. Tests: fulgur `build_tag_group_attaches_a_links_annotations_to_one_group_only` and `render_smoke::test_render_tagged_link_over_several_boxes_does_not_panic` (red before: the krilla panic), ours `tests/tagged_links.rs` (three repros tagged: `Ok`, `qpdf --check` clean, each `/Link` annotation has a `/StructParent` in the `/ParentTree` and exactly one OBJR).
2. *Multicol.* `partition_children_into_segments` walked the DOM children of the container: the wrapper (no geometry) took the place of its children, so `<div style="columns:2"><div style="display:contents"><p>m0</p>...</div></div>` printed 0 of 10 words (main printed them), a wrapper plus a sibling drew `sibling` over `m0`, two wrappers printed nothing. `blitz_adapter::box_tree_children` replaces contents elements by their children (the plain `children` list when there is none); the partition uses it. Tests: fulgur `partition_looks_through_display_contents_wrappers`, ours `c17_a_wrapper_in_a_multicol_container_keeps_the_columns` (six shapes, oracle 0.01 pt, two columns) and a top-level multicol container in 40 % of the fuzz seeds (1000 seeds pass).
   *Audit of the other walks of DOM `children` in fulgur* (grep of all non-test `.children`, plus a probe bank of 55 shapes, wrapped against unwrapped: multicol variants, span-all, forced breaks and page names, `break-inside: avoid` and `break-after`, orphans/widows, abspos and fixed, running elements, floats, tables and nested tables, lists, flex and grid across pages, 1500-word paragraphs): `pagination_layout` (walks prefer `layout_children`; the other reads are DOM facts: forced-break and page-name scans, `has_splittable_children`, the F102 climb), `convert` (`block.rs`, `positioned.rs` use `layout_children` first), `column_css` (the cascade walk is DOM-ordered by design), `outline`, `engine` (DOM semantics: anchors, headings), the test helpers: all correct with wrappers. Every probe agrees with the unwrapped document except two that agree with the *plain* document's own defects: a thead in a wrapper is not repeated (F127) and `column-span: all` across pages loses words wrapper or not (F30d-b, now F161). Not covered: `column-rule` geometry, `<img>` in a wrapper.

Tagged output changes only for tagged documents in which one link spans several paragraphs (before: panic or the broken structure). Fixtures 01-20, the corpus and the scaling variants are not tagged: byte-identical to main (92 of 92, rechecked after both fixes).

**Deviations from the plan.**

- A contents element opens a Parley style span only when it changes the text (`contents_needs_span`: other font / colour / white-space mode, a text decoration, or an `<a>`). The plan wrapped always; Parley trims the whitespace at both edges of every span (F154, also on main for ordinary `<span>`s), so an unstyled wrapper glued words. With this the oracle (wrapper without styles moves nothing, 0.01 pt) holds.
- In flex and grid containers only whitespace that would start a run of text is hidden (a run reaches across a contents boundary). The plan did not foresee it; the fuzz test found it (48 of 1500 seeds).
- The classification includes the `::before` / `::after` of contents children; `flush_inline_pseudos_recursive` also recurses into contents children.
- The fuzz test compares the wrapped render with the plain one (same words, same boxes) and requires at least 97 % of the words in the plain one, instead of "each word exactly once" in both: a `flex-wrap` row in a table cell runs off the page in 4 of 1500 seeds on both renders (F155). The generator adds `gap`, `padding`, `flex-wrap`, `table-layout: fixed` and `p { max-width: 150px }` so that items do not touch and rows do not overflow the page, and never puts whitespace inside the edges of an inline element (F154).
- Chrome constants are offsets from the first word, not absolute positions (the host's Inter differs from the bundled one by a constant).
- New findings: F154 (Parley edge trimming, medium-high), F155 (flex-wrap in a fixed table cell), F156 (decoration on a contents element), F157 (first `li` marker), F158 (larger inline span in a fixed line-height), F159 (leftovers of the plan's non-goals), F160 (`text-transform` on pseudo-element content).

**Not verified.** The fulgur test against the old Blitz. The fuzz test in the debug profile (only `--release` was timed). The spec statements of §2 are still from memory. Pagination of a contents wrapper that straddles a page break was covered only by C14 and the fuzz test.
