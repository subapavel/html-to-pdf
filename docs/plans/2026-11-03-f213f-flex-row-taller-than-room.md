# F213f: a flex row whose item is taller than the room left is not moved and loses words

Date: 2026-11-03. **Status after step 1 (section 7): stop rule hit. None of C1 to C3 holds, and sections 1 to 4 describe the wrong shape; re-plan before any engine or fixture work.** Finding: F213f in `2026-10-15-m0-findings.md` (medium: words lost, warned; the same on main). It is on the *Pilot bar* list in `docs/roadmap.md`, so it goes before the rest of the queue (ADR-0011). Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`, carried by the existing `[patch.crates-io]`. htmltopdf changes only tests, fixture 46, docs and `Cargo.lock`. No upstream PR or issue. Line numbers were read from the fork's working tree on 2026-11-03, while F213e was uncommitted in it. Re-locate them by function name at the commit you build on.

## 1. Problem

A `display: flex` row near the page bottom has a label item and an item taller than the room left. The row stays where it is. The tall item is placed whole below the page edge and its words are printed on no page, with the warning "content extends N pt past the bottom page edge … and is not printed". Chrome prints them. The validator's files (`f213v/c/fmc_if_r250.html`, `c/m211.html`) are lost; rebuild them under `out/scratch/f213f/` (git-ignored). The file name suggests "flex item, monolithic carrier, `inline-flex`, 250 px of room"; that reading is a guess.

Head as F213: `@page{size:A4;margin:15mm}body{font:14px/20px Inter;margin:0}p{margin:0}`, content box 1009.13 px. `S(R)` = `<div style="height:{1009-R}px"></div>`. `B(h,d)` = `<span style="display:{d};width:150px;height:{h}px;border:1px solid #000"><span style="display:block">BOXTOP</span></span>`. `L` = `<div style="width:128px;flex-shrink:0">Poznámka:</div>`. `F(x)` = `<div style="display:flex;gap:16px">x</div>`. Every probe ends with `<p>AFTER</p>`.

| Id | Body | Kind of the tall item |
|---|---|---|
| K1 | `S(250) F(L<div>START B(300,inline-flex) TAIL</div>)` | one-line carrier in an item (the validator's name) |
| K1b | K1 with `inline-block`; K1 with the carrier as the first item | the same, other box / order |
| K2 | `S(250) F(L START B(300,inline-flex) TAIL)` | box written directly in the row: blockified, anonymous items around the text |
| K3 | `S(250) F(L <div>Adresa<br>START B(300,inline-block) TAIL</div>)` | inline root of 2+ lines whose second line is tall |
| K4 | `S(50) F(L <div>3 lines</div>)` | paragraph with no allowed break (ADR-0010) |
| K5 | `S(250) F(L <img 100x300>)`, `F(L <div>10 lines</div>)` | guards: leaf; splittable item |

Invoice shape (fixture 46): a payment row, `<div class="flex items-start gap-6 mt-6 text-sm"><div class="w-32 shrink-0 font-semibold">Platební údaje</div><div>Zaplaťte QR kódem <span class="inline-block border border-gray-300 rounded-lg p-4">…QR…</span> nebo převodem na účet 123456789/0300.</div></div>`, near the bottom of page 1. The final form is whichever kind step 1 finds red.

## 2. Spec and Chrome

Cited from memory; `css-spec-researcher` has not run. All wording and section numbers are **unverified**; step 1 confirms them.

- **css-flexbox-1 §12, Fragmenting Flex Layout.** Flex containers may break between items, between flex lines and inside items. Normative bullet: "If the first fragment of the flex container is not at the top of the page, and none of its flex items fit in the remaining space on the page, the entire fragment is moved to the next page." §12.1 (informative) is the sample algorithm for a single-line row container: each item is fragmented on its own, starting from the container's top.
- **css-break-3 §4.1.** The break in front of the row is a Class A break between siblings in block flow. Class B breaks are between line boxes; there is no break point inside a line box. A UA may treat inline-level boxes that establish a formatting context as monolithic (ADR-0004 relies on this).
- **css-break-3 §4.4.** Rules for unforced breaks, relaxed only when the allowed breaks cannot keep content from overflowing (as quoted in ADR-0010). Monolithic content that does not fit is pushed to the next fragmentainer; only content taller than a page may be sliced (section unverified, as in F213 / F213d).
- **Reading.** The §12 bullet *requires* the move when no item fits; it does not forbid it when the label fits. If an item that cannot break does not fit, keeping the row in place makes it overflow. The Class A break in front of the row avoids that, so css-break-3 chooses it. This is the same reasoning as F213 and ADR-0010 item 1. If the tall item can break (lines, blocks), the row starts in place and the item splits, as today (F30b; F30h P2: Chrome starts such a row in place).
- **Chrome m153** (predicted; measured in step 1 with `scripts/reference/render.mjs` and `pdftotext -bbox`): K1, K1b, K2 (box part), K3 and K4: the whole row on page 2. K5: `<img>` row on page 2; 10-line item starts in place. If Chrome slices or overflows a monolithic item in place instead, step 1 stops; that would need an ADR (spec vs Chrome) before any code.

## 3. Where it belongs: today's mechanism, candidates, design

This is fulgur fragmenter behaviour, in `crates/fulgur/src/pagination_layout.rs`. Taffy lays the row out correctly; Blitz and Parley are not involved.

**How a flex row is fragmented today.** It is neither one monolithic box nor sliced; it is cut through its items.
- The gate (body `:4186`, nested `:6340`) enters the row when `would_split_block_subtree` (`:4832`) sees an item that overflows the room.
- F28 (body `:4250-4287`, nested `first_break_h` `:6564-6587`) first moves the row whole if its first row does not fit. For a row, `leading_block_size_with` (`:4974`, loop `:5013-5150`) returns the maximum, over the items level with the first one, of each item's own leading.
- Inside the row, `RowState` (`:5580`, `:5966-6034`) places every item from the row's start; each item is split on its own (F30b lines, recursion into blocks).
- Only the first item may start a page (`first_in_row`: later items get `first_break_h = 0`, `:6550`, `:6565`).
- An item flush with the row's top cannot be cut (`child_page_y > page_start_y + band`, `:6608`).

So a word is lost when **the row's move test and the walk disagree about an item**: F28's leading for the item is smaller than what the walk places unbroken at the row's top.

**Candidates, ranked from reading.**

1. **C1, inline root of 2+ lines (K3, K4).** Leading takes `abspos_line_split` (`:8980`, `orphans` lines). The walk asks `line_split_candidate` (`:8945`), which has more barriers, and keeps the item whole when that refuses it or no break is allowed.
2. **C2, blockified atomic or fixed box (K2).** Leading descends to its short first child. The walk does not enter it because its children fit, and places it whole.
3. **C3, F213's carrier inside an item (K1).** Expected green, because `monolithic_line_carrier` (`:8616`) already makes leading return the item's height. If K1 is red, the predicate fails for flex items: check `is_inline_root`, the line count, and that the item is not stretched past `fresh`.

Step 1 decides with the widened-MediaBox read-back (F213 §7.2) and a fork dump test of the row's and the item's `fragments`. If none of C1 to C3 holds, stop and re-plan.

**Design.** Rule: a row that something precedes on its page, and that fits a fresh page less `ancestor_insets`, moves whole when one of its first-row items has an unbroken lead that does not fit the room. The unbroken lead is computed by the same predicate the walk uses.

```rust
/// html-to-pdf F213f: block-size, from the row's top, that the block walk places for `item` (an item of the
/// flex / grid row `row`) before the first break it can take inside it.
/// Whole height for a monolithic box ([`monolithic_box`], F213e) and for an inline root the walk keeps whole;
/// the split leading for one `line_split_candidate` accepts; otherwise [`leading_block_size_with`].
fn item_unbroken_lead(doc: &BaseDocument, row: &blitz_dom::Node, item: &blitz_dom::Node,
    fresh: f32, clamp_orphans: bool) -> f32;
```

- **Where it is called.** Only from the row branch of `leading_block_size_with`: when `node` is a flex / grid container whose items do not stack (`!items_stack_vertically`), each first-row child's leading is `item_unbroken_lead`. Reason: both F28 sites and wrappers (F28 via a wrapper's first row) already use this function, so the gates need no new call site.
- **The walk does not change.** Reason: the walk is right once the row moves; changing it would move F30h's shapes.
- **Grid.** The predicate applies to grid rows too, because the code path is shared. If any grid case in the sweep moves farther from Chrome, restrict it to flex.
- **Determinism and cost.** No new map, so output stays deterministic. The line count is checked first (`lines()` without allocating), so `collect_inline_line_metrics` runs only for items of 2+ lines.

**Interaction with F213e, in progress in the same functions.**
- F213e amends `ancestor_insets` (it stops counting the top insets of `<html>` / `<body>`) and adds `monolithic_box`, `first_row_is_monolithic`, `offset_first` and `first_box_overflows`.
- F213f reuses `monolithic_box` and the amended `ancestor_insets`, and edits neither. Reason: two branches in one function would conflict, and F213e's semantics must hold for both.
- F213e's `first_row_is_monolithic` follows the first item of a row only. A row at the top of page 1 under a body margin, whose *second* item is monolithic, is therefore not moved by F213e. Sweep column "body margin" pins this. Fix it in F213f only if it is a one-line change to ask `item_unbroken_lead`; otherwise file it as F213f-b.
- Build on F213e's merged head. If F213e is not merged when step 3 starts, branch from main's pin and rebase after F213e lands (F213d §7.5 did the same with revert / re-apply).

## 4. Acceptance

**Fixture `46-flex-payment-row-page-bottom`** (Czech invoice): the 43 skeleton (header, parties, N rows of 41's table, total), then the §1 payment row, then `<p class="mt-6 text-xs text-gray-500">Děkujeme za spolupráci.</p>`.
- **Shape.** The item's form is the red kind from step 1 (the carrier item if K1 is red; otherwise K3 with a `<br>`, or the box directly in the row).
- **Classes.** Check each class in the minified CSS: `rg -o '\.w-32\{' fixtures/tailwind/tailwind.css` (escape `/` as `\\/`; also try `\.cls:` for variants). `flex-1`, `flex-col`, `grow` and `w-1/2` are absent, so the fixture uses `w-32 shrink-0` and an item without a width class. A missing size goes into the fixture's `<style>`, as in 44. No rebuild.
- **Tuning.** Choose N in step 1 so that, in Chrome and in ours, the room is above the label's height and below the item's lead.
- **PNG must show.** Page 1 ends after the total, with a gap below. Page 2 starts with the whole row (label, text, QR box), then the thank-you line.
- **Asserts** (`fixtures.rs`): `Platební`, `Zaplaťte`, `0300.` and `Děkujeme` once each, all on page 2; every `yMax` at most the content bottom; page count equal to Chrome's (2).
- **Compare.** Record main's per-page %. The fixture is new, so the threshold is the worst page after the fix + 1 point.

**Ours, new `crates/htmltopdf/tests/flex_row_tall_item.rs`.** Every expected page is Chrome's from step 1, and each test names it.
- **Red:** one `r_…` test per kind step 1 finds losing words (K1 to K4 at body, in a `<section>` after a `<p>`, and in a `border p-4` card).
- **Guards (green):**
  - K5 `<img>` (moves);
  - K5 10-line item starts in place (F30h P2);
  - a row that fits stays;
  - a row taller than a page is entered (F213b, ADR-0004);
  - K4 moves whole, naming ADR-0010.
- **`page_bottom_warning.rs`:** the red kinds join "must not warn".

**Fork, red first.**
- Lib tests:
  - `item_unbroken_lead` units (one per arm);
  - `a_flex_row_whose_item_is_kept_whole_and_taller_than_the_room_moves_whole`;
  - `…_in_a_wrapper`;
  - `…_when_the_tall_item_is_second`.
- Unchanged and green: `leading_block_size_of_a_flex_row_is_its_tallest_item`, `flex_row_leaf_cells_move_together_when_the_row_does_not_fit`, the co-split tests.
- A `render_smoke` case.

**Byte gates** (1 and 4 rayon threads; any other change stops the work and a postmortem is written):
- fixtures 01-45 and the 7 corpus documents byte-identical; 46 changes (step 1 lists any fixture with a flex row that crosses a page, and must find none);
- fork `fulgur-vrt`: the same failing set and identical `.actual.pdf` files; the 21 fork examples identical;
- `bench.py --check`.

**Sweep** (`out/scratch/f213f/sweep.py`, ours vs main vs Chrome from the same HTML):
- **Grid:**
  - room {40, 120, 199, 250, 400};
  - kind {K1, K1b, K2, K3, K4, `<img>`, 10 lines, 4 `<div>` lines};
  - position {first, second of 2, middle of 3};
  - align {default, `items-start`, `items-end`};
  - nesting {body, section after `p`, card, grid, `body{margin-top:100px}`};
  - box h {100, 300, 1000}.
- **Pass:** every changed case is on Chrome's pages ("every word on Chrome's page") or closer than main; none is farther; no case loses more words than main. Report rows moved, pages vs Chrome, and warnings before / after.

## 5. Steps (each at most a day)

1. **Measure (no code).**
   - Rebuild the K probes; render Chrome and main; do the widened-MediaBox read-back and write the fork dump test (not committed).
   - Have `css-spec-researcher` verify §2.
   - Grep the fixtures for flex rows near a page edge; size N for fixture 46.
   - Record in §7 which candidate holds. Stop if none does, or if Chrome does not move the row.
2. **Red tests and fixture.** `flex_row_tall_item.rs`, the warning entries, fixture 46 with its `render.mjs` reference and README; commit.
3. **Fork, predicate.** `item_unbroken_lead` and its units, with no call site yet. Byte gate: everything identical.
4. **Fork, wire.** Row branch of `leading_block_size_with`; lib tests, `render_smoke`, clippy, fmt.
5. **Bump and gate.** `cargo update -p fulgur --precise <sha>`; our full tests, byte gates, sweep.
6. **Finish.**
   - `render-fixtures.sh` and `compare.py`; look at 46's side-by-side.
   - Run `vrt-validator`. No `pdf-reviewer`: layout only, no writer path.
   - Update the findings (F213f done, residuals filed) and `docs/upstream.md`; write §7.

## 6. Risks, size, non-goals

- **Size.** Medium, about 5 days. The risk is in step 1: the cause is unconfirmed and the repro is lost.
- **Gaps at the page bottom.** A moved row leaves a gap, as in Chrome. A document may gain Chrome's page; the byte gate and the sweep watch this.
- **F30h P2.** A stretched label beside a splittable item must keep starting in place. Its guard and the sweep's "10 lines" column pin it; moving it would be a regression.
- **ADR-0004 / ADR-0010.** A row taller than a page, and an alone box over `fresh - ancestor_insets`, keep their present paths. A 3-line item moves whole, as ADR-0010 says.
- **F213e conflicts** in the same functions; see §3.
- **Cost.** The predicate runs for every first-row item that reaches F28. It must not allocate for one-line items; `bench.py --check` guards it.
- **Non-goals:**
  - F30h: centred / end-aligned items lost silently; stretched siblings' frames;
  - F96e: item frames on continuation pages;
  - F213b: a line taller than a page;
  - F183c: the y of the box;
  - `flex-wrap` lines after the first;
  - slicing.

## 7. Evidence

### 7.1 Step 1 (2026-11-03): what loses words, what does not

Measured with the prebuilt CLI of main c625feb (fork `b81c6782`, bundled Inter, `--image red60.png=...` for the `<img>` probes) and Chrome m153 (`Skia/PDF m153`, `emulateMedia(print)`, `preferCSSPageSize`, as `scripts/reference/render.mjs`). Words and y from `pdftotext -bbox`; a second read of the same PDF with every MediaBox widened downwards by 3000 pt (F213 plan 7.2) lists what is painted below the sheet. "Lost" = on no page of the unwidened PDF. No cargo was run, no fork code was read at run time (the fork source at `b81c6782` was read with `git show`, nothing built). Probe generators are in the scratchpad `f213f/` (`probes.py`, `grid.py`, `t0.py` to `t7.py`, `sweep.py`); their HTML is under `f213f/p/`. The validator's files still exist in `f213v/c/` (`fmc_if_r250.html`, `m211.html`).

**The plan's K kinds, rebuilt as written (`grid.py`, rooms 40 / 120 / 199 / 250 / 400, `probes.py`):**

| Kind | main | Chrome | Verdict |
|---|---|---|---|
| K1 `F(L <div>START B(300,inline-flex or inline-block) TAIL</div>)`, also carrier first | no word lost, no warning, but the row is **20 px tall** and the box is drawn 20 px tall; the row stays on page 1 | whole row on page 2 (`AFTER` 267.4 pt) | not a fragmentation defect: a layout defect, see 7.3 |
| K2 `F(L START B(300,inline-flex) TAIL)` | whole row moves to page 2, nothing lost | row starts in place, the blockified box is split across pages (`AFTER` page 2 at 79.2 pt) | main moves where Chrome splits; the plan's prediction for Chrome was wrong |
| K3 `F(L <div>Adresa<br>START B TAIL</div>)` | same as K1 (height collapse), nothing lost | whole row on page 2 | as K1 |
| K4 `F(L <div>3 lines</div>)`, room 50 | row moves whole, nothing lost | 2 / 1 split (ADR-0010) | conforming |
| K5 `<img>` (100x300) | row moves, nothing lost | row moves | green, a guard |
| K5 10 lines, stretch | rooms 40 / 120 / 199: **the whole row moves to page 2** | starts in place and splits the item (`line1` on page 1) | not green: the plan's guard "starts in place" does not hold on main |

**The same kinds with the carrier in a block child (`<div><p>START B TAIL</p></div>`), 728-case sweep (`sweep.py`: 14 kinds, rooms 40 / 120 / 199 / 250, nesting body / section after `p` / `border p-4` card / `<section>` / grid, align default / `items-start` / `items-end`, Chrome rendered from the same HTML):**

| Kind (52 cases each) | lost | painted below the content box | page differs from Chrome | warned |
|---|---|---|---|---|
| carrier item + label (`K1p`, `K1pf`, `K1ifp`), 2-line `K3p`, `img` | 0 | 0 | 0 | 0 |
| `K4` (ADR-0010), `d4` (4 block lines), `K2` | 0 | 0 | 13, 9, 51 (Chrome starts in place or splits; main moves whole) | 0 |
| `l10` | 4 (`items-end`, room 199: `line1` to `line7`, silently, no warning) | 0 | 39 | 0 |
| **carrier item + 15-line `columns:2` item** (`MC15`, also `AV_MC15` with `break-inside:avoid` as in the validator's file) | **12** (room 199: `START` `BOXTOP` `TAIL`) | **24** (room 250) | 25 | 24 |
| carrier item + 8-line `columns:2` item (`MC8`) | 0 | 12 (room 250) | 12 | 12 |
| `columns:2` item first, carrier second (`MC15f`) | 0 | 0 | 24 | 24 |

(`MC4` lost 52 only because the expected word list named `CELL8`, which a 4-line probe does not have; ignore that row.) The loss does not depend on nesting (body, section after `p`, card, `<section>`) or on `align-items`; a grid row is green. Room 40 and 120 are green for `MC15`.

**Validator file `fmc_if_r250.html`:** Chrome: `START` `BOXTOP` `TAIL` and every `CELL` on page 2 (y 41.7 pt). Main: `CELL1` at 612.8 pt, `START` `BOXTOP` `TAIL` at 826.6 pt on page 1 (27 pt below the content bottom 799.4, the box cut off; widened read-back agrees), `AFTER` page 2; warnings "a paragraph of 1 lines is placed whole and ends 55px below the content box" and "content extends 209.4 pt past the bottom page edge". With room 199 (`MC15`, body) `START` is at 865 pt, off the 841.9 pt sheet: lost, warning 247.6 pt.

**What the red shape needs.** A flex row with a `columns` item and an item that cannot be broken in front of its end. Replacing the carrier by a leaf makes no difference to the move: `<div>` with an `<img>` 300 px tall next to an 8- or 15-line multicol item, room 250: Chrome has the row, all `CELL`s and `AFTER` on page 2; main keeps the row on page 1 (`CELL1` at 613 pt, the `<img>` below the sheet edge), with the same "placed whole and ends 55px below" warning. The same `<img>` next to a label moves (K5, green). So the **multicol item is what keeps the row from moving**; the carrier is only one thing that then overflows. Fork `b81c6782` says in the walk's multicol branch: "in a flex / grid box, a table or an atomic inline it stays one box, F30d-b", and `leading_block_size_with` returns the node's own height for a multicol container. Neither is the F28 leading mismatch of C1 to C3. Not confirmed by a dump (not built, see 7.4).

### 7.2 Candidates

- **C1 (inline root of 2+ lines, K3, K4): does not hold.** `K3p` and `K4` lose nothing in 52 cases each.
- **C2 (blockified atomic box, K2): does not hold.** K2 loses nothing; main moves the row where Chrome splits the box.
- **C3 (carrier inside an item, K1): does not hold.** With a block child the carrier row moves exactly as Chrome (0 of 52 differ); K1 as written is the height defect of 7.3.
- **New, C4 (not in the plan): a flex row with a multicol item is not moved when another item does not fit.** Red in 24 of 52 `MC15`, 12 of 52 `MC8` cases (room 250), 12 of 52 `MC15` cases lose words (room 199).

### 7.3 Defects seen on the way (not F213f)

- **Fixed-height box directly in a flex item.** `F(L <div>START B(300) TAIL</div>)` at the top of a page: main `AFTER` at 60.1 pt, Chrome 267.4 pt; the box is drawn about 20 px tall. Same for `min-height`, for a box alone in the item, with `align-items:flex-start`, with a width on the item, and with `flex:1`. Not with `<div><p>...</p></div>` in the item, not in a grid, not outside a flex row, and **not for an auto-height box**: the invoice shape (label + `Zaplaťte QR kódem <span inline-block p-4>label, svg, amount</span> nebo...`) is green in all 10 cases tried (rooms 40 to 250, direct and in `<p>`): every word on Chrome's page, no warning. So fixture 46 as sketched in section 4 would not be red.
- **Multicol item in a flex row is as tall as its one-column layout.** `F(L multicol-15)` at room 250: Chrome keeps the row and `AFTER` on page 1 (730.2 pt, 160 px multicol); main keeps the row, `AFTER` on page 2.

### 7.4 Stop rules, fixtures, what was not done

- Stop rule 1 (no candidate holds): **hit.** Stop rule 2 (Chrome does not move the row): not hit, Chrome moves the row in the red shape.
- Fixture scan (Chrome print layout of `fixtures/invoices/*`, rows that are `display:flex|grid`, not in a table cell, not `flex-direction:column`, position approximated from the screen layout at 1009 px per page): only 08's grid section crosses a page (a grid of splittable cards); rows within 300 px of a page bottom: 05 footer (room about 249), 14 totals grid (152), 20 `dl` (299), all single-page fixtures that fit. None has a multicol item in a flex row. Main's byte gate would not need an exception, but nothing was built to prove it.
- Fixture 46 was not sized and not written: no red invoice shape exists. Red tests were not written.
- Not done: the fork dump (no clone, no build; disk was 3.6 GB), `css-spec-researcher` (section 2 is still unverified), `vrt-validator`. The F213e interaction in section 3 was not examined.
- Re-plan input: either narrow F213f to the multicol-in-row shape (synthetic: no fixture or invoice has it) and decide whether it is worth the work, or drop the invoice fixture and close F213f as "not reproducible for invoice shapes".
