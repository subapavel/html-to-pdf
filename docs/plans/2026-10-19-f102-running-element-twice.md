# F102 — an element with `position: running()` is also painted in the body

Date: 2026-10-19. Finding: F102 (`2026-10-15-m0-findings.md`). Status: **done** (§7). Repro: `/tmp/claude-0/pdfrev-f99b/docs/own1.html` (PNG of page 1: `/tmp/claude-0/pdfrev-f99b/own1p-1.png`).
The fix goes on the fulgur fork (`subapavel/fulgur` `htmltopdf/srgb-colours`, after `6a6da9b`), carried via `[patch.crates-io]`. Blitz is not changed: the fix uses its existing `DocumentMutator::set_style_property` (Blitz `mutator.rs:315`). Line references are to `6a6da9b` as read in `/home/user/fulgur`. As in F99b, I could not confirm the checked-out commit with git.

## 1. Problem and trace

- **What we see.** In `own1.html` (rendered without `--css`, as in `run.sh`), the header `<div class="run">` with `.run { position: running(runhdr) }` appears in `@top-center` through `element(runhdr)`. It also appears a second time on page 1, at 10 pt, at the left content edge, just above the `<h1>`. It has its own `/Link`, Rect [87 781 141 793]. No fixture in 01–16 or in `bench/corpus/` uses `running(` (grep), so `compare.py` cannot see this. `determinism_threads.rs` M1/M2 (F99b) render the duplicate too, but they only compare runs of the same binary against each other.
- **Trace (hypothesis, confirmed or refuted in step 0).** There are three ways CSS reaches the main document, and only two of them hide running elements.
  1. **AssetBundle / `--css`.** `parse_gcpm` rewrites `position: running(x)` to `display: none` in `cleaned_css` (`gcpm/parser.rs:647-669`). The engine injects that text (`engine.rs:193-194`, `:307-311`).
  2. **`<link>` / `@import`.** The net provider gives Stylo the cleaned text (`net.rs:142-150`).
  3. **Inline `<style>`.** `extract_gcpm_from_inline_styles` (`blitz_adapter.rs:779-820`, called at `engine.rs:234-235`) only *reads* the text. Its `cleaned_css` is appended to `gcpm.cleaned_css` (`gcpm/mod.rs:482-487`) after `css_to_inject` was cloned at `engine.rs:194`. So it reaches the margin-box documents (`render.rs:4444`) but never the main document (`gcpm/mod.rs:235` says so). Stylo drops `position: running()` as an invalid value, so the element keeps `display: block` and its box.
- **How that box gets painted.**
  - `RunningElementPass` (`blitz_adapter.rs:2256-2301`) serialises and registers the element but changes nothing in the document.
  - The body loop of the fragmenter gives it a zero-height fragment at the cursor on page 1 (`pagination_layout.rs:1406-1426`). That keeps it out of the body height, but the node now has a fragment.
  - `convert_node` / `convert_node_inner` (`convert/mod.rs:386-410`, `:595-623`) have no check for running elements, so a `ParagraphDraw` with a `LinkSpan` is emitted.
  - `render_v2` puts the node into page 0's list (`render.rs:176-185`) and draws the paragraph (`:1643-1666`). The body link collector records the `<a>`.
- **Nested elements also take space.** Below the body, the fragmenter does not skip running markers (`pagination_layout.rs:2842-2846`), and Taffy has sized the parent around the box.
- **Not explained by reading the code.** The copy and its link sit 49–61 pt from the top edge, above the content box (70.9 pt), where `clip_body_occurrences_to_content_area` (`render.rs:370-375`, `link.rs:36-52`) should drop a body link. Step 0 explains both, or stops the plan.
- **Why the existing tests do not catch it.**
  - `gcpm_integration.rs` passes the CSS through the AssetBundle (path 1), where the element is already hidden, and asserts only that a PDF exists.
  - `render_smoke.rs:177-212` uses `<link>` (path 2) and says outright that it does not check the body.
  - `link_integration.rs:128-151` **asserts the bug**: it requires at least 2 URIs, "both source running element and margin-box render".
  - The inline-style snapshots (`gcpm_snapshot.rs:81`, `:104`, `:139`) and the VRT `gcpm/running-element`, `element-policy-{first,first-except,last,start}` are byte goldens taken from renders that had the duplicate.
  - The unit test `pagination_layout.rs:7609` checks geometry only.

## 2. Spec

No `css-spec-researcher` output exists. All references are cited from memory; their section numbers and wording are **unverified**.

- **CSS GCPM 3 §2.2 (running elements).** `position: running(<custom-ident>)` removes the element, with its `::before` / `::after`, from the normal flow and makes it available to `element()`. "The element inherits from its original position in the document, but does not display there." So the source position takes no space and paints nothing. An element that no `element()` places is not displayed anywhere.
- **CSS GCPM 3 §2.2.1 (`element()` and `first` / `start` / `last` / `first-except`).** Which instance a page shows depends on the page its *source position* falls on. So a hidden element still needs a page assignment (§3.2, placeholder).
- **CSS Syntax 3 / Cascade 5.** An invalid declaration is ignored, which is why path 3 leaves the box in place. **Cascade 5 §6.1:** a declaration attached to the element (the style attribute) beats rule-based declarations of the same origin and importance. It loses to an author `!important` rule, hence guard C in §3.1.
- **CSS Display 3 §2.5.** `display: none` generates no box for the element or its descendants. This is how the spec behaviour is implemented here.
- **The reference.** Chrome does not implement `position: running()` or `element()`: it lays the element out in flow and leaves the margin box empty. So the reference is **X_ref = the same document with the running element(s) deleted from the body**.
  - The body of X must match X_ref word for word, at the same positions.
  - The margin boxes of X must show the element on the pages the policy picks.
  - WeasyPrint and Prince both take running elements out of the flow and show them only in margin boxes (unverified, from memory; not a gate).
- **Chrome probes (step 0, not CI gates).**
  - P1: Chrome renders `own1.html`. Expected: the div is in flow and `@top-center` is empty. This records why X itself cannot be the reference.
  - P2: Chrome renders `own1-ref.html` (div deleted). After the fix, our body words must sit within the usual drift (about 3 px) of Chrome's, as in the other fixtures.

## 3. Options, decision, and where it lives

This belongs in **fulgur**. GCPM is fulgur's own layer: Stylo and Blitz do not know `running()`, and teaching Stylo to parse it would be a Stylo fork for one keyword. It does not belong in **ours**: the cause is in fulgur's passes, and `htmltopdf` must not reach into them.

### 3.1 Ranking

| Option | What | Covers | Decision |
|---|---|---|---|
| A | After the inline fold, inject `<sel> { display: none }` for the inline mappings | path 3 only. Loses to a later or more specific author `display` (`.run { position: running(x); display: flex }`, Tailwind `flex` on the element), which already breaks paths 1 and 2 today | Rejected: same weakness as the existing rewrite |
| **B** | `RunningElementPass`, after serialising, sets `display: none` on every registered node as a style-attribute declaration | all three paths and every author normal rule (§2, Cascade 5 §6.1). No new node, so node ids are unchanged | **Chosen** |
| **P** | Post-pass `append_running_placeholders`: a registered node with no geometry gets a zero-height fragment at its source position | Needed with B. A nested `display: none` node is zero-size and is never recorded (`record_descendant`, `pagination_layout.rs:2402`), so `collect_running_element_states` (`:5585-5622`) would never see it and its margin box would stay empty | **Chosen** |
| **C** | `convert_node` returns early for a registered node | guard against author `!important` display (beats B's normal declaration). No drawables, link or tag | **Chosen** (cheap: one lookup in an empty `BTreeMap` for documents without running elements) |
| D | Remove the node from the DOM after serialising | No geometry left for the page assignment; the later passes walk the node | Rejected |

### 3.2 Design (signatures only)

- `gcpm/running.rs`: `impl RunningElementStore { pub fn node_ids(&self) -> impl Iterator<Item = usize> + '_ }`, in `BTreeMap` order.
- `blitz_adapter.rs`: `fn hide_registered_running_elements(doc: &mut HtmlDocument, store: &RunningElementStore)`.
  - Called at the end of `RunningElementPass::apply`, after `walk_tree`, so `serialize_node` has already copied the HTML. `set_style_property` changes `style_attribute`, not `attrs`, so the margin HTML would not change anyway.
  - It runs before `resolve` (`engine.rs:324-331`, `:462`).
- `pagination_layout.rs`: `pub fn append_running_placeholders(geometry: &mut PaginationGeometryTable, doc: &BaseDocument, running_store: &RunningElementStore)` and `fn running_placeholder_fragment(geometry: &PaginationGeometryTable, doc: &BaseDocument, node_id: usize) -> Option<Fragment>`.
  - Applies only to registered nodes **without** geometry; body-level ones keep their fragment from `:1406`.
  - Position rule:
    1. Take the nearest previous sibling (layout order) that has geometry: use the page and bottom of its last fragment.
    2. Otherwise use the parent's first fragment (page and top), going up the ancestors.
  - The rule mirrors the cursor of the body loop. Zero height, so `implied_page_count` cannot grow.
  - Called in `engine.rs` after `:610` (after every geometry pass), before the anchor map.
- `convert/mod.rs:386`: `convert_node` returns early when `ctx.running_store.instance_for_node(node_id).is_some()`.
  - Margin documents use an empty store (`render.rs:4737`), so they are unaffected.
  - If R7 shows that `walk_semantics` (`:454`) still creates entries for the subtree, the same guard goes there.
- Unchanged:
  - the `display: none` rewrite in `cleaned_css`, and `strip_display_none` (`render.rs:4853-4857`). Changing them would move margin-box CSS for every GCPM document (side finding F104).
  - `abspos_boxes_in_tree_order` (`abspos_flow.rs:106`) already skips running nodes.

### 3.3 Byte identity by construction

- **No running rule anywhere:** fixtures 01–16, `bench/corpus/`, every non-running VRT fixture, every example except the header-footer pair. `RunningElementPass` is not built (`engine.rs:325`), the store is empty, P loops zero times and C is a miss. These must stay **identical**.
- **Running rule from `--css` / `<link>`, element directly under `<body>`, rewrite not overridden:** `gcpm_integration`, AssetBundle snapshots, `examples/header-footer*`, `link_integration` cases with an AssetBundle. Computed `display` is already `none`, the geometry already exists, so they are expected **identical**. `cmp` decides.
- **Expected movers:**
  - Every inline-`<style>` running document: the VRT `running-element`, `element-policy-first`, `-first-except`, `-last`, `-start`; snapshots `gcpm_running_element_via_inline_style`, `gcpm_element_policy_first`, `gcpm_element_policy_last`; M1/M2 in `determinism_threads.rs` (green either way).
  - Nested running elements on paths 1 and 2: their margin box goes from empty to filled. That is content that was missing.
  - A golden may be updated **only** if `pdftotext` shows the duplicate in the old one: the running text appears once more on page 1 than on page 2 (for VRT), or more than once (single-page snapshots). Any other mover stops the step (§5).

## 4. Acceptance

- **`crates/htmltopdf/tests/running_elements.rs` (new).**
  - Each case is generated with a `with_running: bool` switch, giving X and X_ref (§2).
  - Helpers:
    - `words(pdf) -> Vec<Word{page,x0,y0,x1,y1,text}>` (`pdftotext -bbox`, as in `body_offset.rs`)
    - `link_count(pdf) -> usize` (occurrences of `/Subtype /Link`; annotations are uncompressed, as fulgur's `link_integration.rs` relies on)
    - `content_band(page)`
  - Each case asserts:
    - (a) X and X_ref have the same page count, and the same words at the same boxes (±0.5 pt) inside the content area;
    - (b) the running text is never inside the content area on any page;
    - (c) the running text is in the margin band on exactly the pages listed below;
    - (d) the link count in X equals X_ref's plus one per page whose margin box shows a link.

  | Case | Shape | Margin pages |
  |---|---|---|
  | R1 | `own1.html` trimmed, header with `<a>`, 3 pages; three variants of the rule: inline `<style>`, `--css`, `<link>`, whose word lists must be equal | 1–3 |
  | R2 | header, footer and a third running element `@bottom-left element(ico)`, each with a link | 1–3 each |
  | R3 | `element(hdr, first-except)`, and a second element whose source position is on page 2 with `element(kap, first)` | hdr 2–3; kap 2–3, not 1 |
  | R4 | running footer containing a `<table>` (IČO / IBAN), a bundled PNG `<img>` and Tailwind `flex justify-between` (`--css tailwind.css`). Also `.run { position: running(f); display: flex }` in one rule, and `display: flex !important` on the element through `--css` | 1–2; `pdfimages -list`: page 1 has as many images as page 2 |
  | R5 | nested: running div inside `<header><div>`; and a running element inside another running element (outer placed) | outer 1–3, with the inner text inside it; inner text nowhere in the body |
  | R6 | running element between paragraph 30 and 31 of one `<section>` that splits over pages 1–2, `element(x, first)` | 2 only; no gap in the body (a) |
  | R7 | R1 with `tagged: true` | same `/Type /StructElem` count as X_ref tagged; zero `/S /Link`. The margin link stays untagged (F20) |
  | R8 | `position: running(x)` with no `@page` / `element()` | none; `link_count(X) == link_count(X_ref) == 0` |

- **fulgur fork.**
  - Lib: `running_pass_hides_registered_elements` (inline `<style>`; after the pass and `resolve` the node has `display: none` and zero size; `serialize_node` output unchanged); `running_pass_beats_a_later_display_rule`; `running_placeholder_for_nested_element` (wrapper with `break-before: page`: one zero-height fragment on page 1); `running_placeholder_leaves_body_level_fragment_alone`; `convert_skips_running_node_with_important_display`.
  - `render_smoke`: a one-page document with an inline running header that contains a link has exactly one `/Subtype /Link`; the tagged variant has no `/S /Link`.
  - `link_integration.rs:146-150`: change the assertion to `uri_count == 1` and fix the message. This is the test that encoded the bug.
  - `gcpm_integration`, `examples_determinism` 10/11 as before.
- **Fixture 17 (`fixtures/invoices/17-running-footer/`): add it.**
  - Why: the invariant asks for a fixture that shows the pixel change, and none of 01–16 can. It is the realistic GCPM invoice shape: company details and IBAN on every page. Tailwind cannot declare it, because `parse_gcpm` ignores rules inside `@layer` (F105), so the inline `<style>` path, the broken one, is the path users will hit.
  - Shape: two pages, about 45 item rows, Czech content.
    - Inline `<style>`: `.paticka { position: running(paticka) }` and `@page { @bottom-center { content: element(paticka) } }`.
    - The footer: `<footer class="paticka flex justify-between ...">` with "Příkladná integrace s.r.o. · IČO 12345678 · DIČ CZ12345678" and "IBAN CZ65 0800 0000 1920 0014 5399 · <a>www.example.cz</a>".
    - Use only classes already in the committed `tailwind.css` (grep first). If one is missing, set that property with an inline `style` instead (do not rebuild Tailwind; precedent fixtures 12, 13, 15).
  - Reference: `scripts/reference/render.mjs` renders `reference.html` when the fixture has one: footer deleted from the body, its two halves as `@bottom-left` / `@bottom-right` strings. The README says why.
  - The PNGs must show the footer once per page in the bottom margin and nothing above the first heading.
  - `compare.py`: new threshold = worst page + 1 point, with a comment.
- **Bytes and pixels.**
  - Fixtures 01–16 and `bench/corpus/`: `cmp`-identical to main at `RAYON_NUM_THREADS=1` and `4`, and `compare.py` numbers unchanged (pixel-diff change 0).
  - `own1.html`: the 10 pt copy and one `/Link` on page 1 are gone.
- **Reviews.** `vrt-validator` with no high-severity finding. `pdf-reviewer` on R7's PDF and fixture 17, because annotations and the structure tree change.

## 5. Steps (each one day or less, each testable on its own)

0. **Diagnosis and gate (half a day).** Render `own1.html` with the main binary in three ways: as it is; with the `.run` rule moved to `--css`; with `@top-center` removed.
   - The hypothesis predicts: duplicate; no duplicate; duplicate.
   - Explain the 49–61 pt position and why the link survives `link.rs:36`.
   - Run probes P1 and P2 (§2), and run `pdftotext` on the 5 VRT goldens and 3 snapshots to list the ones that show the duplicate.
   - **Stop** if the second render still duplicates or the third does not, or if the link comes from the margin collector. Then write a postmortem in `docs/postmortems/` (10 lines: probes and outcomes) and re-plan against `MarginBoxRenderer`.
1. **Red tests (one day).** `running_elements.rs` R1–R8, the fork lib and smoke tests, the `link_integration` flip. Record which are red.
2. **B (half a day).** `node_ids`, `hide_registered_running_elements`. Expected green: R1, R2, R4 (except `!important`), R7, R8. Run `cmp` on fixtures 01–16.
   - **Stop** if anything without a running rule moves (postmortem).
3. **P (one day).** `append_running_placeholders` and the engine wiring. R3, R5, R6 green; `pagination_layout` unit tests green.
4. **C (half a day).** Guard in `convert_node` (and in `walk_semantics` if R7 needs it). R4's `!important` variant green.
5. **Fork goldens and push (half a day).** Update only the goldens listed in step 0. Any other mover stops the step. Run `fmt`, `clippy -D warnings`, lib, `render_smoke`, `gcpm_*`, `link_integration`, VRT, `examples_determinism`, then push.
6. **Ours (one day).**
   - Lock bump, fixture 17 (`index.html`, `reference.html`, README, references, `render.mjs`), the threshold, the `cmp` matrix, `compare.py`; look at `out/compare/17-*.png` and `02-long*.png`.
   - Docs: `docs/upstream.md`, F102 marked done, new findings F104–F106, §7 here.
   - Then `vrt-validator` and `pdf-reviewer`.

## 6. Risks and non-goals

- **Author `!important` `display`** beats B. C keeps it out of the paint, but a nested element could still take space. That is accepted and covered by R4.
- **Changed behaviour that is correct per §2, but visible:**
  - An `id` on a running element no longer gives `href="#id"` a destination; it used to come from the body copy.
  - A running heading no longer adds an outline entry when bookmarks are on.
  - Record both in §7.
- **Placeholder rule.** A nested element exactly at a page break gets the page of the previous sibling. R6 checks this; the body-level path is unchanged.
- **Inheritance.** The margin copy inherits from the margin box, not from its original position (GCPM §2.2). This is a deviation that already exists and is not changed here.
- **Side findings to file:**
  - F104: `strip_display_none` removes every author `display: none` from margin-box CSS, for example Tailwind `hidden`.
  - F105: `parse_gcpm` recognises `running()` only in top-level simple-selector rules: not inside `@layer` / `@media`, not with compound selectors, not in `style=""`.
  - F106: no warning when a running name is never placed by `element()`, so the content silently disappears (R8).
- **Non-goals:** F103 (vertical centring of top margin boxes), F20 (tagging margin-box content and links), `element()` for an element nested in another running element (it is not registered), `string-set` / counters inside running elements, and everything in F104–F106.

## 7. Result

fulgur fork `182775c` (fix and tests) and `c2d07c9` (goldens), on `6a6da9b`, pushed; `Cargo.lock` pins `c2d07c9`. Blitz unchanged (`d4258c6`).

### 7.1 Step 0, the gate (main's binary `2e29308`, `own1.html` with all Inter faces)

| Render | Prediction | Result |
|---|---|---|
| as it is (rule inline) | duplicate | duplicate: a 10 pt copy at y 48.6, 4 `/Subtype /Link` (3 margin + 1 body), `<h1>` at 74.1 |
| `.run` rule moved to `--css` | none | none: 3 links, `<h1>` at 56.1 |
| `@top-center` removed | duplicate | duplicate: 1 link, no margin box at all, `<h1>` at 74.1 |

The link comes from the body collector (the third render has no margin box and still has one). **The gate passed.** Two corrections to §1:
- **The 49-61 pt position is not above the content box.** `RenderOptions::default()` gives the page a uniform 15 mm margin (`Margin::uniform_mm(opts.page.margin_mm)`, `lib.rs:385`) and that wins over the document's `@page { margin: 25mm 15mm }` (F6). The content box starts at 42.5 pt, plus the 6 pt body margin: 48.5 pt, a 12.1 pt line is 48.6-60.7. So `clip_body_occurrences_to_content_area` was right to keep the link: it was inside the real content box. The "70.9 pt" in §1 was read from the CSS. Tests use the real band (15 mm), not the `@page` one.
- **The copy takes room.** The `<h1>` sits 18 pt lower with the copy than without (74.1 against 56.1): Taffy laid the box out, and only the fragmenter's cursor counted it as zero height. So the body moved, not only grew a duplicate.
- It also showed that the body copy is only *painted* when the running element is itself an inline root: the descendants of a body-level running element have no geometry, so a footer made of a flex container with a table and spans printed no text in the body on main, only its `border-t` and its room (fixture 17 on main: a rule above "Faktura" and the page 34 px low).

Chrome probes: P1 (`own1.html`): the div in flow at y 73.8, `@top-center` empty, as expected. P2 (div deleted, `@page` margin 15 mm to match our page): first paragraph at 91.0 against ours 93.6 (box tops), the usual drift; with the document's own 25 mm margin the comparison is 25 pt off for F6's reason, not this one.
`pdftotext -bbox` of the eight old goldens: every one showed the running text in the body, but not always as a "duplicate": `running-element` and `element-policy-{first,last}` print it twice on the pages the margin shows it, `-first-except` and `-start` print it **once, in the body**, on pages whose margin box is empty or shows another instance (the old file looked right to the text check). The criterion "once more on page 1 than on page 2" in §3.3 was too narrow; the rule applied was: the only words that disappear are body copies and nothing is added (commit `c2d07c9` lists them).

### 7.2 Red to green

`running_elements.rs` (ours) against `6a6da9b`: 11 of 12 red, green on the change. The one that was green before is `r1_..._renderer_css` (rule from `--css`, already hidden: the plan's "identical" case). Mutation: with `append_running_placeholders` off in `engine.rs`, `r5_nested_running_elements` goes red and the other 11 stay green (R6 passes without it: the fragmenter evidently records that nested element itself, which §3.1 did not expect; only R5's element, inside `<header><div>`, needs the placeholder).
Fork: `running_pass_hides_registered_elements`, `running_pass_beats_a_later_display_rule` (green after step 2); `running_placeholder_for_nested_element` (step 3), `convert_skips_running_node_with_important_display` (step 4, needed the `walk_semantics` guard too); `running_placeholder_leaves_body_level_fragment_alone` was green throughout (it guards the body-level path); render_smoke `inline_style_running_header_has_exactly_one_link` and `tagged_inline_style_..._no_link_structure` red to green, `linked_stylesheet_..._one_link` green throughout (path 2); `link_integration` `margin_box_running_element_link_keeps_pdf_annotation` flipped from `>= 2` to `== 1`.

### 7.3 Checks

| Check | Result |
|---|---|
| fork `cargo fmt --check`, `clippy -p fulgur --all-targets -- -D warnings` (local Blitz) | clean (one pre-existing Blitz warning in the path crate) |
| fork lib | 2160 passed |
| fork `render_smoke` / `gcpm_integration` / `link_integration` | 266 / 18 / 7 (+1 ignored) passed |
| fork `gcpm_snapshot` | 16 passed, 3 changed as predicted, updated |
| fork VRT | 5 of 64 differ, exactly the five predicted; updated; the other 59 and the three gradient harnesses pass |
| fork `examples_determinism` | 10 of 11, the one failure is `committed_svg_matches_rendered` (host fonts for SVG text, unrelated, "10/11 as before") |
| `examples/header-footer`, `header-footer-split` | byte-identical to a render of `6a6da9b` (their rules reach Stylo through the AssetBundle) |
| ours `cargo fmt --check`, `clippy --release --locked --all-targets -- -D warnings` | clean |
| ours `cargo test --release --locked --no-fail-fast` | 464 passed, 0 failed, 3 ignored, 44 binaries (corrected by the main session from the `vrt-validator` re-count: the first count of 357 was a miscount; 452 on main + 12 new `running_elements` tests) |
| fixtures 01-16 and 7 `bench/corpus` PDFs against main, `RAYON_NUM_THREADS` 1 and 4 | `cmp`-identical (23 of 23 at each) |
| `scripts/compare.py` | 01-16 unchanged (same numbers as before: 4.4, 11.4, 5.7, 5.7, 21.6, 15.9, 7.2, 13.2, 5.1, 5.9, 6.9, 14.6, 11.7, 7.5, 8.7, 20.8 worst page); 17 new |
| `scripts/bench.py --check` | ok; 17-running-footer 36.6 ms median; `b200-items` 1511 ms, `b200-text` 870, `b200-mixed` 884 |

### 7.4 Goldens (all in fork `c2d07c9`, evidence in its message)

`gcpm/running-element` (header and footer twice on page 1, body 45.0 / 45.8 pt up, page 2 text 0.75 pt down), `gcpm/element-policy-first` (title in the body of every page, 30.0 / 21.8 pt), `-first-except` (heading in the body on pages 1 and 3, where the margin box is meant to be empty), `-last` (Intro A, Intro B, Chapter Two in the body, 52.5 / 21.8 pt), `-start`; snapshots `gcpm_running_element_via_inline_style` ("Running Header" twice), `gcpm_element_policy_first` (**6 pages to 3**) and `_last` (**5 to 3**): the title copy took room in a 400 x 300 pt page, so each 220 pt spacer moved to a page of its own.

### 7.5 Fixture 17 (`17-running-footer`)

Two pages, 42 rows, footer once per page in the bottom margin, nothing above "Faktura" (looked at `out/compare/17-running-footer-p1.png`, `-p2.png`). Reference: `reference.html` (footer deleted, two `@page` strings), 2 pages. `compare.py`: **main's binary 12.7 / 10.0 %, now 12.0 / 10.0 %** (the rule above the heading and the 34 px shift are gone); threshold 0.13 (worst page + 1 point), comment in `compare.py`. The rest is the usual drift and the `<tr>` `border-b` rows (F22).

### 7.6 Deviations from the plan

- `<link>` variant of R1 is a fork test (`linked_stylesheet_running_header_has_exactly_one_link`), not in `running_elements.rs`: `htmltopdf` has no base path, so it cannot load a `<link>`.
- R4's `!important` case uses a text element (`display: block !important`), and does not assert (a): an author `!important` `display` beats the declaration (it keeps its room, which is the §6 risk, accepted), and a nested footer's descendants are never painted anyway (§7.1), so only a text element shows the problem. A stronger fix would add the hiding as `!important`; Blitz's `set_style_property` has no importance parameter, so it needs a Blitz change. Not done.
- R4's image check is 0 or 1 per page, equal on both pages: an `<img>` in a margin copy is not painted at all (filed as F107). R5 also asserts the inner text is in the margin copy of the outer element on every page.
- `walk_semantics` got the same guard as `convert_node` (the plan said "if R7 needs it": the unit test needed it, R7 did not).
- The step 2 `cmp` on 01-16 was done once at the end, with all three changes, not after step 2 alone.
- Not run: `vrt-validator` and `pdf-reviewer` (§4 asks for them; the implementing session has no way to start those agents). R7's PDF and fixture 17 still need `pdf-reviewer`.
- Not done: `bench/baseline.json` has no row for fixture 17 (the default limit of 100 ms applies).
- Filed beyond the plan: F107.
