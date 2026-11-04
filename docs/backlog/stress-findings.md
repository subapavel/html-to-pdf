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

### B15 — a margin-box item taller than the band is not clipped; the box grows and drops its sibling text  (severity: low, size: unknown — fulgur)

A 200 px image (or a plain 200 px `<div>`, same on main) in `@top-left` with a 56.7 px band: the box grows to 220 px, the sibling text is dropped and the image paints over the body (F107 validator, `f107v/p/tall.html`, `tall-div.html`). F139 territory.

### B16 — `content: element(h) counter(page)` drops the counter  (severity: low, size: unknown — fulgur)

Same on main and after F107 (`f107v/p/c2.html`); an image and the page number in separate boxes work.

### B17 — an unbundled `list-style-image` / `background-image` in a margin box is dropped without a warning; `content: url()` in a margin box does not paint  (severity: low, size: unknown — fulgur)

F107 validator `f107v/p/sidefx.html`. Nothing is read from the host. Goes with F177c.
