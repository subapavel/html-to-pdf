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

