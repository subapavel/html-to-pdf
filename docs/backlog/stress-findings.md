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

### B23 — inline-blocks that share a line taller than a page stack below the first box instead of beside it  (severity: low, size: medium — fulgur pagination, with F222 / F213b)

F183c re-validation, `rv/var2/four_staggered`: four inline-blocks on one line, each starting on a later page. Every word is printed once and in order, but each later box continues below the previous one's end (B00 on page 3, C00 on page 5: 6 pages, as on main), where Chrome places them beside the first box (B00 on page 2: 4 pages). Same output as main.
