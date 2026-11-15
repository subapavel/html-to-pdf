# Stress-probe findings (backlog)

Low and low-medium findings whose only repro is a synthetic stress shape (ADR-0011). Same entry format as
`docs/plans/2026-10-15-m0-findings.md` (`### Fnnn — title  (severity: …, size: …)`). Move an entry to the queue when
a fixture, a corpus document or an invoice-like layout hits it.

### B1 — a relative shift of thousands of px in a tall multi-column box loses every word  (severity: low, size: unknown — fulgur)

Seen in the F198j-f step 1 sweep (`e_huge_shift`, 2026-11-03): every binary, F198j included, loses all 2500 words; Chrome paints them where the shift puts them. Synthetic: no invoice shifts content by more than a page.

### B2 — a multi-column item in a flex row keeps its one-column height  (severity: low-medium, size: medium — fulgur, F30d-b)

Seen in the F213f step 1 sweep (2026-11-03, plan `2026-11-03-f213f-flex-row-taller-than-room.md` §7): with a label beside a `columns: 2` item at 250 px of room, Chrome keeps `AFTER` on page 1 and ours pushes it to page 2. Same root as F213f (a multicol container in a flex item stays one box).

### B3 — a carrier beside a multi-column item at 310 px of room warns 164.4 pt past the page edge although every word is on Chrome's page  (severity: low, size: unknown — fulgur)

Seen in the F213f sweep (2026-11-03); not investigated. A spurious warning, no content lost.

### B4 — margin 0, a `top: -20px` wrapper around a line taller than a page in columns loses 12 words (4 at 15 mm margins)  (severity: low, size: medium — fulgur, F198l)

Seen in the F198j-f step 1 sweep (2026-11-03). Part of F198l (a line taller than a page is not sliced).

### B5 — an alone inline-block under a collapsed child `margin-top` keeps the F30f split  (severity: low, size: unknown — fulgur, F213d/F213e)

`<p style="margin-top:500px">` holding an alone page-high inline-block: the collapsed margin is counted by `ancestor_insets`, so the box is split where Chrome moves it whole (F213e engineer, 2026-11-03).

### B6 — a page-high box on its correct page paints its last line 0.1–3.2 pt into the bottom margin  (severity: low, size: unknown — fulgur / Parley, F183c)

12 synthetic probes after F213e (validator `f213ev/below.py`); at `@page{margin:0}` the box's bottom border is cut. Caused by the F183c seat.

### B7 — a monolithic first box that overflows page 1 by 0.35–0.87 px is moved; Chrome keeps it  (severity: low, size: unknown — fulgur)

Body padding / border, a float before the box or wrapper padding (F213e validator `f213ev/win2.py`, `w2_bodypad_40_0.5`). Sub-pixel capacity difference (1009.13 vs 1010 px).

### B8 — `@page :first { margin-top }` + body margin + a page-high image in a `<p>`: 3 pages against Chrome's 2  (severity: low, size: unknown — fulgur, F109)

F213e validator `fam4/pf80_img_fit1_bodymt.html`. Pagination against page 1's box (F109) combined with the F213e move.

### B9 — a `<button>` with an explicit height has its label at the bottom of the box  (severity: low, size: unknown — Blitz / fulgur, F183c)

Same on main and after F213e (`a4_button_40_-1`); after F217 the label is no longer centred and the box hangs 15 px over the next block. Goes with F183c.

### B10 — a text-only flex item with an explicit `width` larger than its container is not shrunk  (severity: low-medium, size: unknown — Taffy / Blitz)

`width:400px` in a 300 px flex container stays 400 px; Chrome shrinks it (`flex-shrink: 1`). `flex: 0 1 300px` and a block child do shrink. Same on main (F217 plan §7.6). Move to the queue if an invoice hits it.

### B11 — an `inline-grid` with `min-height` and text is 1.5 pt taller than Chrome  (severity: low, size: unknown — Taffy / Blitz)

102.0 vs 100.5 pt, unchanged by F217.

### B12 — a Blitz unit test reads a process-wide log and fails with the default thread count  (severity: low, size: unknown — Blitz fork tests)

`svg_log_tests::a_rejected_inline_svg_is_a_warning` passes with `--test-threads=1` only.

### B13 — the per-box whole / painted decision of F198j-f depends on flow order  (severity: low, size: unknown — fulgur)

`o_a4m0_3c_PDT` 344 → 411 words missing against F198j round 1 (main 2444), `o_a4firstz_3c_PDT` 592 → 637, `o_a4m0_2c_PTP` 320 → 327 (F198j-f validator, `f198jfv/r2/b`). Never worse than main. The comparison counts lines, not words.

### B14 — cards placed whole by F198j-f are farther from Chrome's page count than F198j round 1  (severity: low, size: unknown — fulgur)

17 of 28 changed probes (`s2/neg_a4m0_n6_e1_t300`: Chrome 1 page, round 1 1, now 2). The cost of printing main's words; a per-line slice would remove it (F198l).

### B15 — a margin-box item taller than the band is not clipped; the box grows and drops its sibling text  (severity: low, size: unknown — fulgur)

A 200 px image (or a plain 200 px `<div>`, same on main) in `@top-left` with a 56.7 px band: the box grows to 220 px, the sibling text is dropped and the image paints over the body (F107 validator, `f107v/p/tall.html`, `tall-div.html`). F139 territory.

### B16 — `content: element(h) counter(page)` drops the counter  (severity: low, size: unknown — fulgur)

Same on main and after F107 (`f107v/p/c2.html`); an image and the page number in separate boxes work.

### B17 — an unbundled `list-style-image` / `background-image` in a margin box is dropped without a warning; `content: url()` in a margin box does not paint  (severity: low, size: unknown — fulgur)

F107 validator `f107v/p/sidefx.html`. Nothing is read from the host. Goes with F177c.

### B18 — a nested `overflow: hidden` block as the last child of an inline-block keeps its own line baseline  (severity: low, size: unknown — Blitz)

F183c plan §7.5: `atomic_inline_baseline` tests `overflow` on the box itself only, so an inline-block whose last child is an `overflow: hidden` block sits on that block's last line, as fulgur's old walk did. CSS 2.1 §10.8.1 and css-align-3 §9.1 would have the clipped block give its bottom edge. Not measured in Chrome; synthetic.

### B19 — `overflow-x: clip` alone on an `inline-flex` is not checked against Chrome  (severity: low, size: unknown — Blitz)

F183c plan §7.5: `clip` is treated like `visible` for the baseline (measured on an `inline-block`: Chrome keeps the last line baseline). A single axis of `clip` on an `inline-flex` was not measured.

### B20 — an `inline-flex` with `overflow: hidden` gives its bottom edge as the baseline; not checked against Chrome  (severity: low, size: unknown — Blitz)

F183c plan §7.5: `first_line_baseline` returns none for `hidden`, `scroll` and `auto` (the box falls back to its bottom edge, as fulgur's old walk did). Chrome was measured for `inline-block` only.

### B21 — `align-items: baseline` in flex and grid: Blitz gives Taffy no item baselines  (severity: low, size: unknown — Blitz / Taffy)

Non-goal of F183c (plan §6). Items aligned by baseline are aligned as if their baseline were the top edge (`start`), so a badge beside a larger text in a `flex items-baseline` row sits higher than in Chrome. Synthetic until a fixture uses `items-baseline` with different font sizes; then it goes to the queue (ADR-0011).

### B22 — an inline-block with `margin: -6px 0` gives 3 pages against 2; with `line-height: 0` AFTER is 3 pt low  (severity: low, size: unknown — Blitz / fulgur)

F183c validator, probe `adv_a5_10_ib_negmargin_auto_Rpage2_d-6` (`f183cv/adv/`): an auto-height inline-block with a negative vertical margin at the page end makes a third page where Chrome has two, and a box with `line-height: 0` puts the block after it 3 pt lower than Chrome. A negative margin on an inline box shortens the line by the margin in Chrome; Parley places the margin box and Blitz keeps the margin in the layout (F183c, commit `514d51e0`), so the line height is probably taken from the border box. Not on a fixture; synthetic.

### B24 — a first paragraph of two or three lines with room for one line under a body offset hangs; Chrome splits it 1 / rest  (severity: low, size: small — fulgur pagination)

F213e-c step 7 (plan `2026-11-09-f213e-b-avoid-card-under-letterhead.md` section 8): `body { margin-top: 980px }`, then `<p>Line1<br>Line2</p>` (room for one 20 px line on page 1). Chrome m153 breaks the paragraph between its lines (first line on page 1, the rest on page 2: orphans and widows give way, css-break-3 section 4.4) when nothing precedes it; ours keeps both lines on page 1 and prints the second over the bottom margin (the F213e-b mechanism, for a paragraph that is not `break-inside: avoid`). After a zero-height sibling both engines move the paragraph whole (fixed in F213e-c). Needs a room of less than `orphans + widows` lines under an offset of 970 px or more on A4, so synthetic; not on a fixture.

### B25 — after a zero-height sibling, a paragraph with `orphans: 1` and one line of room moves whole  (severity: low, size: small — fulgur pagination)

F213e-c validator (PR #101), `rv2/cases/para-m990-n3-ow1.html` (also n = 2, 4, 6, and `para-m940-n4-ow3` with orphans and widows 3): Chrome and main put the first line on page 1 ([1, 3]); after F213e-c the paragraph moves whole ([0, 4]). Main already does this with a 2 px box before the paragraph: the move-whole site does not consult `orphans` / `widows`. No word is lost.

### B26 — `break-before: avoid` / `break-after: avoid` next to a zero-height sibling: the avoid card moves whole where Chrome splits it  (severity: low, size: small — fulgur pagination)

F213e-c validator, `rv2/cases/bb-z-card-bbavoid`, `bb-zbbafteravoid-card`, `z-card-pageavoid`: Chrome 40 / 6, ours 0 / 46 (main hung it 43 / 1 and lost 2 lines; main with a 2 px box: as ours). No word is lost.

### B27 — `@page :first` with a larger top margin: a 45-line card under an offset takes 3 pages where Chrome takes 2  (severity: low, size: unknown — fulgur pagination)

F213e-c validator, `rv2/cases4/first-o200-n45-z.html`: ours [0, 45, 1], Chrome [0, 46], main [43, 1] with lines lost. The first page's content height seems to be used for the later pages. No word is lost.

### B28 — a `height: 0` box drops its overflowing in-flow text  (severity: low-medium — text lost, synthetic shape only, size: unknown — fulgur pagination / render)

F213e-c validator, `rv2/cases2/ov-only.html`: `<div style="height:0"><p>OVERFLOWTEXT…</p></div>` prints nothing on main and after F213e-c; Chrome prints the text, and so do we with a 1 px box.

### B29 — an `inline-block` with a fixed height taller than a page and a short child is one oversized leaf: 193 px of its frame are not drawn and the next line sits at the top of the next page  (severity: low, size: medium — fulgur pagination, F213d-b residual)

Seen in the F213b step 1 probes (2026-11-14, plan `2026-11-14-f213b-split-box-sharing-a-line.md` section 7.2, P2): `<p>START <span style="display:inline-block;height:1200px;width:100px;border:1px solid"><span style="display:block">BOXTOP</span></span> TAIL</p><p>AFTER</p>`, also with the box alone on its line. Chrome m153 has the box on pages 1 and 2 and `AFTER` at 187.8 pt on page 2; ours prints every word on page 1 and `AFTER` at 43.6 pt on page 2, with the warning "content extends 102 pt past the bottom page edge". The gate of `fragment_block_subtree` enters a box only when a child overflows the strip (`would_split_block_subtree`), so a box whose children are short is emitted whole and overflows page 1; a plain `div` of the same height is cut by the root-level strip slicer and is right. Not a lost word (since F183c); the box frame and the position of what follows are wrong.

*Note (2026-11-15, F213b validator round 1):* the frame that is not drawn is warned now, as main's overflow was: "the frame of a box of 2602px that is taller than a page is drawn 21px high, 2581px of it are not drawn" (`fragment_tall_box`, for a box on a tall line of a paragraph of several lines; `page_bottom_warning.rs`). The oversized leaf itself is not fixed.

### B31 — fuzz documents that still lose tokens main prints after F213b  (severity: low-medium — words lost, synthetic shapes only, size: large — fulgur pagination / render)

F213b validator round 4 (2026-11-15; plan `2026-11-14-f213b-split-box-sharing-a-line.md` sections 7.13 and 7.14; fork `54689723`): of the validator's fuzz sets (`gen.py` 1716 documents, `gen2.py` seeds 7, 11, 23 and 31 and `gen3.py` seeds 11 and 23; the scripts are in the coordinator's scratchpad, not committed) 8 documents of the first 5716 still print fewer of some token than main (gen 0, gen2 3 + 2, gen3 2 + 1), and 3 of the 800 of the fresh seed 31. Classes: (1) a checker artefact, `gen2/f7_0375` (main prints every word twice, 7.5 pt apart; the branch once, 1 pt past the right edge); (2) multi-column containers after a tall paragraph whose columns hold a box wider than the page, so the words are in the PDF but past the right edge (`gen2/f7_0432`, `gen2/f7_0555`, `gen3/f11_0047`, `gen3/f11_0452`; seed 31 `f31_0447`, `f31_0611`): a fallback keyed on "a box wider than its column" fixes two of them and regresses ten others, so none is shipped; (3) an `inline-block` with a negative `margin-top` that starts a column or a page (`gen2/f23_0011`, `gen2/f23_0656`, `gen3/f23_0035`): the clamp to the page top is shipped limited to the room below the content, and fixes only a reduced repro, because where the page is full it trades the first line for the last. The root of 2 and 3 is the same: the multicol container of the branch starts a page of its own (round-2 rule) where main's starts below the text before it; a better rule for the container is the next step.
