# F12 — Repeat `<thead>` on continuation pages

Date: 2026-10-16. Finding: F12 in `2026-10-15-m0-findings.md`. Status: **done** (§7). Fix goes on `subapavel/fulgur`
`htmltopdf/srgb-colours` (base `8d81949`, fulgur 0.40.0), carried via `[patch.crates-io]`.

## 1. Problem

A table that spans pages prints its header row only on the first page. In `fixtures/invoices/02-long` (60 rows,
3 pages) Chrome repeats `# · Popis · Množ. · Cena/h · Celkem` at the top of pages 2 and 3; we start straight with
row 21 (`out/compare/02-long-p2.png`), so the columns are unlabelled. The red test
`crates/htmltopdf/tests/table_header_repeat.rs` (120 rows, `pdftotext -f N -l N`) fails on pages ≥ 2.

fulgur 0.40.0 says this is not done: `drawables.rs:216-218` (`TableEntry` doc) and `render.rs:2836-2840`
(`draw_table_v2` doc); the v1 path that did it is gone. ADR-0001 and the M0 findings (line 17) claim fulgur
repeats `thead` — wrong for 0.40.0; corrected in this PR.

## 2. Spec

- CSS 2.1 §17.2, `table-header-group`: "Print user agents may repeat header rows on each page spanned by a table.
  If a table contains multiple elements with 'display: table-header-group', only the first is rendered as a header;
  the others are treated as if they had 'display: table-row-group'." Same wording for `table-footer-group`.
  HTML §15.3.8: `thead { display: table-header-group }`.
- CSS Tables 3 (fragmenting tables): header and footer groups may be repeated in each fragment. *(Section number
  unverified.)*
- CSS Fragmentation 3: breaks between table rows are class A break points; a repeated header takes block-size from
  the fragment.
- Chrome (TablesNG) repeats a header group only if it is unbreakable and at most ¼ of the page tall. *(Unverified;
  we adopt it as the policy because it also prevents header-only pages.)*
- PDF: ISO 32000-1 §14.8.2.2 (artifacts); PDF/UA-1 §7.1 — a repeated header is a copy and must not add a second
  set of TH structure elements.

## 3. Design (fork)

Blitz lays a table out as a grid whose `layout_children` are the **cells only**, in DOM order; `thead`, `tbody`,
`tr` are flattened (`blitz-dom-0.2.4/src/layout/table.rs`) and get zero size. A table that does not fit goes through
`fragment_block_subtree` (`pagination_layout.rs:1622`); cells of a row share `location.y` and are stretched to the
row height, so the first cell of a row decides the cut at `:2189-2215`.

The fragmenter makes the decision and records it in the existing geometry table; render paints the copy.
`PaginationGeometryTable` and `render_v2` keep their signatures.

| File (fork) | Change |
|---|---|
| `blitz_adapter.rs` | `first_table_header_group(doc, table_id) -> Option<usize>`: the first child with `display: table-header-group` (by display value, not tag name). |
| `pagination_layout.rs` | `RepeatingHeader { group_id, cells, top, height }` and `repeating_table_header(doc, table_id, page_height_px)`: `Some` only if the header cells are a prefix of the table's `layout_children`, `0 < height <= page_height / 4`, and the table has no overflow clip. |
| `drawables.rs` | `table_header_groups: BTreeMap<NodeId, Vec<NodeId>>` (header group → drawable NodeIds under it, sorted). Update the `TableEntry` doc. |
| `convert/table.rs` `convert_table` | For the header-group child: `draw_mark()` before `collect_table_cells`, store `drawn_since(mark)` after. |
| `render.rs` | Extract the per-node body of `draw_v2_page` into `dispatch_page_node(...)` (no output change). Add `paint_repeated_table_headers(...)` called in `render_v2` after `draw_v2_page`. |

**Fragmenter** (`fragment_block_subtree`, parent is a table with a repeating header):

- When the first header cell is placed, push the source fragment `geometry[group_id]` =
  `{page, x, y: child_page_y, w: parent_w, h: height}`; repeat fragments carry `is_repeat = true` so `is_split()`
  stays false.
- Every page advance while placing a **non-header** cell: reserve the band (`cursor_y = child_page_y = height`,
  `page_taffy_origin = this_top_in_parent - height`), push `{new_page, y: 0, h: height}` to `geometry[group_id]`,
  and set a per-page `floor = height` so the overflow test at `:2189` becomes `child_page_y > page_start_y + floor`
  (a row taller than `page − header` is not cut again on every page).
- Applies at the overflow cut (`:2189-2215`), break-before (`:1969-1991`, `:1888-1907`) and break-after
  (`:2151-2167`, `:2251-2267`, `:1924-1942`). Not where a cell's own content split across pages (`:2065-2148`):
  that page gets no header (safe degradation, no overlap).
- Counter / string-set / bookmark collectors read `fragments.first()`, so a repeat never fires them twice.

**Render** (`paint_repeated_table_headers`): for each `(group, ids)` whose geometry has a fragment `k > 0` on this
page, translate by the difference between the repeat fragment's and the source fragment's page position (margins
from `resolve_page_settings` per page, `body_offset_pt` on page 0), then `dispatch_page_node` each id as if on the
source page. A separate canvas with tag / link / bookmark collectors off; with tagging on, wrapped in
`ContentTag::Artifact(ArtifactType::Other)` (not `Header`, which krilla maps to `/Pagination /Attached [/Top]`).
No links in the copy. Everything keyed by `BTreeMap`.

## 4. Acceptance

- `crates/htmltopdf/tests/table_header_repeat.rs`: ≥ 3 pages, every page contains `Hlavička Cena`, every row printed
  exactly once, `Hlavička` once per page (proves space is reserved — no row hidden under the band).
- Fork unit tests (`pagination_layout.rs`): one header fragment per page; on pages ≥ 1 the first body cell starts
  at the header height; no body cell overflows the page; header taller than ¼ page → no repeat; `tbody` before
  `thead` → no repeat; a row taller than `page − header` stays on one page.
- Fork `tests/render_smoke.rs`: a tagged 3-page table has as many `/TH` as a 1-page one and contains
  `/Artifact BMC`; PDF/UA on, header with a link and an inline `<span>`: no panic.
- Fork goldens: only `examples/table-header/index.pdf` changes (re-bless with the pinned `FONTCONFIG_FILE`); all
  other goldens byte-identical (also proves the `dispatch_page_node` refactor is output-neutral).
- Fixture 02: pages 2–3 start with the header; 3/3 pages. Today 10.3 / 11.8 / 9.6 %. Expected to drop modestly
  (rows stay one row off Chrome because of Blitz #386). Lower the threshold to the new worst page + 1 point; if a
  page grows, merge-policy item 3 (missing content now rendered) applies and the PR names the numbers.
- Fixtures 01, 03–05: byte-identical.

## 5. Steps

1. Fork, fragmenter + unit tests.
2. Fork, convert: `Drawables::table_header_groups`.
3. Fork, render: output-neutral `dispatch_page_node` refactor first, then `paint_repeated_table_headers`, smoke tests,
   re-blessed golden, updated docs. Steps 1–3 land together (step 1 alone leaves blank bands).
4. Ours: lock bump, test green, compare + threshold, `docs/upstream.md`, findings, ADR-0001 note; `vrt-validator`
   and `pdf-reviewer` (tagging/artifacts).
5. (Optional, separate) keep the header with the first body row at a page bottom.

## 6. Risks and non-goals

- Rows whose cells split internally: no header on that page (pre-existing split-row gap). `tr { break-inside }` is
  not read (rows are not walked).
- Nested tables repeat their own header inside their recursion; outer header not repeated on pages reached through
  a cell's own split.
- Not repeated: tables with `overflow: hidden|clip`; absolutely positioned content in `thead`; links in the copy.
- `tfoot` repetition deferred as F12b (Blitz places footer cells in DOM order; needs bottom reservation).
- `draw_table_v2` paints the full `layout_size` frame on every fragment (pre-existing; no effect on Tailwind tables).
- Rebasing onto newer fulgur: `fragment_block_subtree` changes often; keep the change behind one
  `Option<RepeatingHeader>` in few hunks.

## 7. As landed

- Fork `subapavel/fulgur` `htmltopdf/srgb-colours`: `ba074a2` (record header-group drawables), `cbc8412`
  (output-neutral `dispatch_page_node` refactor), `cbf62b5` (fragmenter + render + tests), `67d8007` (re-blessed
  `examples/table-header/index.pdf`). Fork: 1895 lib tests, 213 render_smoke tests (4 new), clippy clean; all other
  example goldens byte-identical (checked with a fontconfig that rejects the sandbox's host Inter, which otherwise
  breaks `committed_svg_matches_rendered` at the base commit too).
- Deviations: `HeaderRepeat` (no `top` field; source fragment written lazily with the first repeat, so tables that
  never repeat leave geometry untouched); repeat also disabled when the table or an ancestor clips, transforms or is
  translucent (the copy would bypass those wrappers); a header that itself spans pages is not repeated; break-before
  on the first body row does not create a header-only page.
- Fixtures: 01, 03, 04, 05 byte-identical. 02 3/3 pages, 10.3 / 11.8 / 9.6 → 10.3 / 11.5 / 9.9 %; page 3 grew 0.3
  because the header band is content that was missing (merge-policy item 3). Threshold 0.129 → 0.125.
- After `vrt-validator` (PR #13), fork `84e58df` narrows the repeat: (1) a row that fits a page but not
  page − header gets the whole page, no band there (it was split); (2) only `<table>` elements repeat (a
  `display: table` div got a blank band, since convert records header drawables only for `<table>`); (3) a header
  left alone at the bottom of its page is not repeated (it showed twice). Repros `out/scratch/{tallrow990,divtable,
  near1000}.html`: divtable and near1000 byte-identical to pre-F12, tallrow990 keeps the tall row whole and repeats
  the header on the next page. Fork 1898 lib + 213 smoke tests, clippy clean, table-header golden unchanged.
- Known, not fixed (low): the band can cover a wrapper div's continuation border (cosmetic); repeat silently
  skipped inside `overflow: hidden` / transform / opacity wrappers (Tailwind `overflow-hidden rounded-lg` around a
  table is common — worth a warning later); a page that only carries a split cell's tail shows a bare header band.
