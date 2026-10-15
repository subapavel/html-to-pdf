# F5 — Table column widths differ from Chrome (02 paginates to 4 pages)

Date: 2026-10-15. Finding: F5 in `2026-10-15-m0-findings.md`. Status: **root cause found, fix verified locally, parked
— needs the Blitz fork** (same blocker as F1: forks cannot be created from agent sessions).

## 1. Root cause

Not "Taffy has no table algorithm" in general: Blitz builds the column tracks from the first row's cell widths and
always adds the cell's horizontal padding (`blitz-dom-0.2.4/src/layout/table.rs:175-180`, marked
`TODO: account for padding/border/margin`). Tailwind's preflight sets `box-sizing: border-box`, where `width` already
includes the padding. In 02 every fixed column is 8–16 px too wide (`w-8 pr-2` → 40 instead of 32, `w-16 px-2` → 80
instead of 64 …), the auto description column loses about 48 px, descriptions wrap, and the invoice runs to 4 pages.

`table-layout: fixed` (the mitigation the finding proposed) changes nothing: it only stops later rows from widening
columns; the first row is still padded twice.

## 2. Fix (upstream, one hunk)

```diff
--- blitz-dom-0.2.4/src/layout/table.rs
@@ fn collect_table_cells
                     taffy::CompactLength::LENGTH_TAG => {
                         let len = style.size.width.value();
-                        let padding = style.padding.resolve_or_zero(None, resolve_calc_value);
-                        style_helpers::length(len + padding.left + padding.right)
+                        // With `box-sizing: border-box` the width already includes padding and border.
+                        if style.box_sizing == taffy::BoxSizing::BorderBox {
+                            style_helpers::length(len)
+                        } else {
+                            let padding = style.padding.resolve_or_zero(None, resolve_calc_value);
+                            style_helpers::length(len + padding.left + padding.right)
+                        }
                     }
```

(Border widths under `content-box` are still ignored — add them in the same commit.)

## 3. Measured with the patch applied locally (`cargo --config 'patch.crates-io.blitz-dom.path=…'`)

| Fixture | Pages before → after | Worst page diff before → after |
|---|---|---|
| 02-long | 4/3 → **3/3** | 10.6 % → 12.1 % (rows now sit next to Chrome's but drift by Chrome's 1 px collapsed row borders, Blitz #386) |
| 03-logo-qr | 1/1 | 10.1 % → 9.7 % |
| 01, 04, 05 | unchanged | unchanged |

## 4. Steps once the Blitz fork exists (together with F1's blitz-dom change)

1. Add the hunk above as a commit on `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4` (the one branch that carries all
   our blitz-dom fixes, since `[patch.crates-io]` takes one source per crate).
2. `[patch.crates-io] blitz-dom`, remove `02-long` from `PAGE_COUNT_MAY_DIFFER`; 02's threshold rises per the standing
   decision (content now in place, drift is #386) — name it in `compare.py`.
3. Acceptance test: a two-column table, `box-sizing: border-box`, first column `width: 100px; padding: 0 20px`, the
   second column starts at 100 px.
