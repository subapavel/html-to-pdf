# F66i: the `::before` / `::after` of an inline element nested in another inline element are lost

Date: 2026-10-30. Finding: F66i in `2026-10-15-m0-findings.md` (HIGH, text lost, pre-existing on main). Closes F159(d) (same cause, found "from reading" in F150) and, if step 4 confirms it, F66j (§4.3). Fork: `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`, head `ba908646` (the commit in `Cargo.lock` after F66f §7.3). Line numbers were read in the local checkout `/home/user/blitz`; re-read them before editing.

## 1. Problem

Repro `out/scratch/f66fv4/c3/nobr_nested.html` (A4, 15 mm, `font:14px/20px Inter`): `<div><span>a <span class=t></span> w</span></div>` with `.t::before{content:'tx'}`. Chrome m141 prints `a tx w` (`nobr_nested.chrome.pdf`). Main and the F66f head print `a w`, with no warning. With the host directly in the block (`<div>a <span class=t></span> w</div>`) both print `a tx w`. Generated content in a nested inline is a common template idiom: a currency after an amount (`<td><span class=price><span class=amount>1 250,00</span></span></td>`, `.amount::after{content:' Kč'}`), icons and separators in badges and links (`<a><span class=ext>…</span></a>`), footnote and required-field markers (`label span::after{content:'*'}`). Losing them is silent data loss. F66j (`ps_*_after_br`, `p2_Apl_*`) is a consequence of the same bug: Chrome puts AFTER at 73.8 pt, main at 73.6 (by accident), the F66f head at 58.6 (`out/scratch/f66fv4/res.txt`, `res2.txt`).

Which shapes lose content, from reading §3 (step 1 confirms each against Chrome):

| shape | today |
|---|---|
| host is a direct inline child of its line-layout root (`div`, `p`, `td`, `li`, a flex/grid item, the inside of an inline-block) | printed |
| host is nested in one or more inline elements (`span`, `a`, `b`, `strong`, `em`, `label`, …) of a real-element root, at depth ≥ 2; `::before` and `::after` alike; empty host or host with text | **lost**: no box at all (no text, padding, border, background, inline-block box) |
| same, but the root is an anonymous block (the inline run sits next to a block sibling) | printed (by accident, §3) |
| every element between the root and the host's parent is `display: contents` | printed (F150) |
| host is itself an atomic inline (`inline-block`, `inline-flex`, `button`) at any depth | its own pseudo-elements are printed; depth then counts again from that box |
| `content` other than a single leading string (`counter()`, `attr()`, quotes, `url()`, `"a" counter(x)`) | empty box or first string only, **at any depth**: separate (§7) |
| `display: block` pseudo-element of an inline element | laid out as an atomic inline box at any depth: separate (§7) |

## 2. Spec (cited by the planner, from memory; css-spec-researcher not run, section numbers unverified)

- **CSS 2.1 §12.1**: `:before` / `:after` "interact with other boxes … as if they were real elements inserted just inside their associated element". Nothing depends on where that element sits in the tree, so a nested inline's pseudo-element is a box like a direct child's.
- **css-pseudo-4 §4.1** (generated content pseudo-elements): `::before` / `::after` exist when `content` computes to something other than `none` / `normal`; they are inline-level unless styled otherwise.
- **css-content-3 §2** (`content`): strings, counters, `attr()`, quotes, images. Only strings are in scope here (§7).
- **css-display-3 §2.5** (`display: contents`): the element's pseudo-elements still exist. This is the F150 behaviour; it must not regress.
- **PDF/UA-1 (ISO 14289-1) §7.1 / WCAG F87**: generated text that carries meaning (`Kč`, `*`) is real content, so it must not be an artifact. Chrome tags it as part of the surrounding text.

## 3. Cause, traced (Blitz fork, `packages/blitz-dom/src/layout/construct.rs`)

Pseudo-element boxes are created only by `flush_pseudo_elements` (433-515). The inline walks only *read* `node.before` / `node.after`: `find_inline_layout_embedded_boxes_recursive` (853, 869) and `build_inline_layout_recursive` (1220, 1241), at any depth. For a root with inline children only, the creation path is:

1. `collect_layout_children(div)` (133) flushes the root's own pseudo-elements (148), takes the inline-root branch (261-277) and calls `find_inline_layout_embedded_boxes` (740).
2. That calls `flush_inline_pseudos_recursive(root)` (773-792). It flushes every child (775), so depth 1 works. It then decides whether to descend into the child by reading **`doc.nodes[node_id]`, the parent** (776-778), not `child_id`. For a block, table-cell or flow-root root this is never `(Inline, Flow)`, so it never descends past depth 1 unless the child is `display: contents` (F150, 779-782).
3. The walk at 838-878 removes the construction damage of every inline element (851), so `collect_layout_children` never runs for the nested span either. Its pseudo-element is never created; 853 and 1220 read `None`.

An anonymous block works by accident: its computed `display` is the initial `inline` (`create_anonymous_block`, 663-697), so at its level the parent test passes, and below it every inline-flow parent passes too. A `display: contents` chain works through `child_is_contents`.

fulgur needs no change. It takes the pseudo-element's text from the Parley layout like any span's (brush = pseudo node id; `DUMMY_NAME` maps to no struct element, `tagging.rs:74-105`, so in a tagged PDF the text lies in the paragraph's marked content, as for a depth-1 pseudo-element today). fulgur's own pseudo-element code (`convert/pseudo.rs`, `inline_root.rs:60-81`) looks only at the inline root's `before` / `after` (images, abspos) and is not involved.

## 4. Fix

### 4.1 Where it belongs

Blitz: box construction is engine behaviour. Fork `subapavel/blitz` branch `htmltopdf/blitz-dom-0.2.4`, a commit on top of `ba908646`. No upstream PR or issue (standing decision of 2026-10-15), no vendored source. htmltopdf bumps the two Blitz lines in `Cargo.lock` and updates the Blitz row in `docs/upstream.md`.

### 4.2 Change

One helper, used by `flush_inline_pseudos_recursive` and by both walks, so that "which pseudo-elements are created" and "which are read" cannot diverge again:

```rust
/// Whether the inline walks descend into `node`'s pseudo-elements and children: an inline-level `flow` element that is
/// not laid out as an atomic inline (`img`, `svg`, `input`, `textarea`, `button`) and is not `<br>`, or a
/// `display: contents` element.
fn inline_walk_descends(node: &Node) -> bool;
```

In `flush_inline_pseudos_recursive`, keep the flush of every child (775, unchanged) and replace 776-787 with `if inline_walk_descends(&doc.nodes[child_id]) { recurse }`. The two walks use the helper in place of their own match arms (838-878, 1189-1256); their behaviour does not change. The predicate must be the child's display, not a superset of the old parent test. Reason: the old test also descended into atomic inlines and `display: none` subtrees under an anonymous block. Those are flushed later by their own `collect_layout_children` or never rendered, so keeping that descent would only keep a wrong rule. Fallback if the byte gate (§5) moves a fixture *only* through node-id order (pseudo nodes created earlier or later): OR the old parent clause back in and record why in §8.

### 4.3 F66j: folded in, no code

F66j's shapes put a nested span with a pseudo-element after the `<br>` inside a padded span. Once the pseudo-element exists, `build_inline_layout_recursive` emits `BoxStart { edge }` / `EndEdge` / text for it (1210-1255), and the F66f §7.2 rule (`line_content_after_last_break`) already treats that as content. So F66j needs tests, not code. If a shape still differs after the fix, it stays in F66j, and this PR adds no code for it (one finding per PR).

## 5. Acceptance

### Ours: new `crates/htmltopdf/tests/nested_pseudos.rs`

Helpers as in `trailing_break.rs` (`render_with`, `printed_words` via `pdftotext -bbox`, extended with `x_min`) and `tagged_links.rs` (`render_tagged`, `check_and_qdf`). Head `@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}td{padding:0}`. Two oracles per case. (a) The words in order, and `x_min` within 1 pt of Chrome (measured in step 1, recorded in §8). (b) For shapes with no padding, the depth-n render's words and x positions equal the depth-1 render's within 0.01 pt. Chrome gives both the same layout, so this holds with or without Chrome numbers.

1. `the_pseudo_of_a_nested_span_is_printed`: the repro gives `a tx w`. Red on main.
2. `before_and_after_at_depth_two_to_four`: `::before`, `::after` and both, on an empty host and on a host with text, at depth 2, 3 and 4. Red.
3. `every_root_kind`: roots `div`, `p`, `td`, `li`, a flex item, and the inside of an `inline-block`. Red.
4. `hosts_that_templates_use`: `a`, `b`, `strong`, `em`, `label`, including `label span::after{content:'*'}` and `.amount::after{content:' Kč'}` in a `td`. Red.
5. `a_box_without_text`: `content:''` with `display:inline-block;width:6px;height:6px;background:#000` at depth 2. The next word's x equals the depth-1 render's. Padding and border variants likewise. Red.
6. `trailing_space_in_content`: `content:"Pred "` at depth 2 keeps its space (as F154 W10). Red.
7. `f66j_pseudo_after_a_br`: the nine `ps_before_after_br`, `ps_after_after_br`, `ps_before_border_after_br`, `p2_Apl_{t,t2,tb,te,tl,tp}` shapes. AFTER within 1 pt of Chrome's 73.8. Red (58.6).
8. `tagged_nested_pseudo_text`: a tagged render of tests 1 and 4 passes `qpdf --check`, and `pdftotext` has `tx`, `Kč`, `*`. Red.
9. Guards, green on main: depth 1; the anonymous-block shape; a `display: contents` chain; `<q>` at depth 2 (an empty box as at depth 1: quotes are §7); `counter()` and `url()` at depth 2 equal depth 1. No panic, same words.

### Blitz fork

`construct.rs` test module `nested_inline_pseudo_tests`, using the `table.rs` `pseudo_tests` harness. After resolve, the root's `TextLayout::text` contains `tx` for depths 2-4 and for roots block / table-cell / inline-block. The host has `before.is_some()`. A depth-2 `inline-block` pseudo-element is in the root's layout children. Red before the fix. Also a guard: a pseudo-element under `display:none` inside an anonymous block is not read.

### Fixture `35-nested-generated-content`

A Czech invoice. All pseudo-element rules are in an inline `<style>` (as fixture 22), so `tailwind.css` is not rebuilt. It holds:

- item amounts `<td class=num><span class=price><span class=amount>1 250,00</span></span></td>` with `.amount::after{content:" Kč"}`;
- a status badge `<p>Stav: <span class=badge><span class=dot></span>Uhrazeno</span></p>`, where `.dot::before` is a 6 px inline-block green dot;
- `<p>Datum zdanitelného plnění: <strong><span class=req>15. 10. 2026</span></strong></p>` with `.req::after{content:"*"}`, and a legend `* údaj podle § 29 zákona o DPH`;
- a note in Czech quotes `<span><span class=q>…</span></span>` with `::before{content:"„"}` / `::after{content:"“"}` (string literals, not `<q>`: §7);
- `<a href="https://platba.example.cz"><span class=ext>platba.example.cz</span></a>` with `.ext::after{content:" →"}`.

Check the glyphs: `strict_fonts` fails on a glyph Inter lacks; replace that glyph. The Chrome reference comes from `scripts/reference/render.mjs`. `fixtures.rs` asserts every generated string, in order, with x within 1 pt of the reference. The PNG must show the Kč amounts right-aligned in their column, the green dots, the asterisk and the quotes as in Chrome. On main all of them are missing, so the diff before the fix is large; the compare threshold is the worst page + 1 point.

### Byte gates (1 and 4 rayon threads, `out/scratch/f66fv2/gate.sh`)

- **Fixtures 01-34**: byte-identical to the `ba908646` binary. Reason:
  - their element pseudo-elements (fixtures 21, 22, 23) sit on block, section and table boxes, which `collect_layout_children` flushes;
  - `tailwind.css` has no `content` declaration, so preflight `::before` / `::after` rules create no pseudo-element (Stylo `servo/selector_parser.rs:278-288` `should_exist`);
  - no fixture uses `<q>` (Blitz UA `q:before{content:open-quote}`), and `img::before{-moz-alt-content}` is gecko-only and not parsed.
- **Corpus (7 documents)**: byte-identical (only `@page` margin-box `content`).
- **Fork `fulgur-vrt`**: `toc-target-counter*.html` use `a::after` on a link directly inside `<li>` (depth 1), so no golden should move; do not re-bless on this host (35 of 64 fail for host fonts, `docs/upstream.md`). Compare the `.actual.pdf` files before and after.
- **The F66f sweep and the `f66fv4` batches** (`batch5.py`, 1331 documents): a changed document must contain a nested inline pseudo-element and be closer to Chrome; anything else stops the work.

## 6. Steps

1. Measure Chrome m141 for tests 1-7 and fixture 35 (x and y of each word), and run the same shapes at depth 1 in ours. Record in §8. If depth 1 itself differs from Chrome for a shape (abspos, block pseudo-element), assert only oracle (b) for it and name the finding.
2. Write the red tests and fixture 35 with its reference, and commit. Tests 1-8 red, 9 green.
3. Blitz: `inline_walk_descends` and the change in `flush_inline_pseudos_recursive` and the two walks. Blitz tests red first. Then `cargo test -p blitz-dom --lib` from the fulgur workspace (forks patched in by `--config`, as in F66f), clippy `-D warnings`, fmt. Push, bump `Cargo.lock`, run our tests and the byte gate. Stop and write a postmortem on a moved fixture 01-34.
4. F66j: rerun the `ps_*` and `p2_Apl_*` batch (`out/scratch/f66fv4/run.py`). Every shape at Chrome means F66j is done in this PR; otherwise the remaining shapes stay in F66j.
5. Finish:
   - run `render-fixtures.sh` and `compare.py`, and look at fixture 35's side-by-side;
   - `vrt-validator`; `pdf-reviewer` is not required (no writer, font or metadata change; test 8 covers tagging);
   - `bench.py --check` (one extra tree walk per inline root, no shaping);
   - findings file: F66i done, F159(d) done, F66j done or reduced, the new finding of §7;
   - `docs/upstream.md`; §8 of this plan.

## 7. Risks and non-goals

- **Node-id order.** New pseudo nodes take slab ids that later anonymous blocks would have taken, in documents that have nested pseudo-elements. Construction is serial (`&mut BaseDocument`), so it stays deterministic; the 1- and 4-thread gate checks it.
- **Tagging.** The new text has no struct element of its own and lands in the paragraph's marked content, like depth-1 pseudo text. Where the paragraph has none (F200, F20, F85), it has none either. It is not marked as an artifact, which is right for PDF/UA because it is content.
- **More text now reaches known gaps.** F160 (`text-transform` on pseudo text: the text node has no `parent`, 480) now applies to nested pseudo-elements too. `scan_siblings` (1439-1440) now sees them as inline content; this is the F66j direction.
- **Not changed (file the first two as findings):**
  - `content` beyond a single leading string (`flush_pseudo_elements` 475-487: counters, `attr()`, quotes, `"a" counter(x)`). This is a new finding, low-medium, Blitz.
  - `content: url()` on a nested inline: fulgur reads pseudo-element images only on the inline root (`inline_root.rs:60-81`; F64 family).
  - An abspos pseudo-element of a nested inline: `inline_root.rs:418-427` suppresses it, and `walk_absolute_children` (`positioned.rs:433-435`) re-emits only the root's. Unverified; measure in step 1, F170 family.
  - A `display: block` pseudo-element of an inline element is not split around (`is_or_contains_block` ignores pseudo-elements, `node.rs:197-241`; css-inline-3 §3).
  - Pseudo-elements of table rows and row groups (F163b); `dir` (F66k); F66l.

## 8. Evidence

(Filled in by the implementer: Chrome measurements, results table, gates.)
