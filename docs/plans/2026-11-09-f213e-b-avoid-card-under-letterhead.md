# F213e-b / F213e-c: a `break-inside: avoid` card at the top of a letterhead page

Date: 2026-11-09. Findings: F213e-b and F213e-c in `2026-10-15-m0-findings.md` (medium, content lost; both on the *Pilot bar* list). Fork: `subapavel/fulgur` `htmltopdf/srgb-colours`, existing `[patch.crates-io]`. Line numbers were read from the fork's working tree on 2026-11-09. F183c is about to be committed to the same branch, so re-read them at the build commit.

## 1. Problem

Take `body { margin-top: 200px }` (or a 200 px card margin collapsed through body) and a first `<div style="break-inside:avoid;border:1px solid #000">` of 45 `<br>`-separated 20 px lines. Our card hangs: lines 42-43 are below the content box, 44-45 are below the page edge and lost ("22.6 pt past the bottom page edge"). Chrome m153 splits it in place, 40 / 5 (F213e-b). With a zero-height `<div>` before the card, Chrome moves the whole card to page 2. Ours hangs it the same way (F213e-c).

Tests already exist, ignored, in `crates/htmltopdf/tests/body_offset.rs`: `f213e_c5_…`, `f213e_c6_…card` and `f213e_c5z_…`.

The path, read from the branch, is not the one the finding names:

- `avoid_inside_holds` (`:1854`) is true for a 900 px card, so `line_metrics` is emptied (`:4013-4018`). The `para_total_h.max(keep_size)` advance (`:4182`) is never reached.
- The gate does not enter the card: its children are text nodes with no height.
- The plain advance (`:4534`) moves only when `cursor_y > 0.0` or `first_box_overflows` holds, and neither does.

So the card is placed whole at `cursor_y = 0` on page 0, whose room is only `bottom(0) = page_h - first_offset`.

Cards of block children already split like Chrome. `keeps_whole_at(…, content_above = false)` (`:4347`) lets the gate enter them. The inline-root card has no equivalent: its avoid decision ignores whether anything precedes it.

F213e-c is the same blindness on the other side. A zero-height sibling leaves `cursor_y` at 0 (`:3723-3799` places it and `continue`s), so `content_above` (`:4320`, `cursor_y - gap > 0.5`) and every `cursor_y > 0.0` guard read "nothing precedes".

## 2. Spec

css-spec-researcher was not run. The wording is from memory and still **unverified**. Step 1 (section 7) measured Chrome m153 against the behaviour it predicts; it did not check the spec text.

- **css-break-3 §4.1.**
  - Class A break points lie between *in-flow* sibling boxes in the block flow. Height is not a condition, so a zero-height block sibling gives a Class A point in front of the card.
  - Class B break points lie between line boxes.
  - Class C break points lie between a container's content edge and its child only when there is a non-zero gap. A margin collapsed through `<body>` is not such a gap.
  - So a first card has no break point in front of it.
  - **Corrected by step 1 (P10, 7.8):** Chrome does not take a Class C break in front of a first avoid card under `padding-top` or `border-top` of `<body>`, `<html>` or a wrapper `div` either (even 1 px of padding): 40 / 5, like a bare first card. Only a preceding sibling box (an element, 7.5) gives the break. A gap above the card, whatever its source, is not "something precedes".
- **css-break-3 §4.4.**
  - Rule 4 (`break-inside: avoid` on an ancestor) and Rule 3 (orphans / widows) are dropped, in the spec's order, when the remaining breaks cannot keep content from overflowing.
  - First card: the only breaks are Class B ones inside it, so Rule 4 is dropped and the card splits in place (F213e-b).
  - After a sibling: the Class A point avoids the overflow without violating Rule 4, so nothing is relaxed and the card moves whole (F213e-c).
  - For a monolithic box F213e took the break in front (the "may break anywhere" end of the list). For a breakable box a Class B break inside it comes first.
- **css-break-3 §5.2.** Margins at an unforced break truncate. The moved card starts at page 2's content top; Chrome puts its first line at 44.6 pt with the bundled Inter (ours 44.3 pt). The 41.7 pt of the F213e tests was measured with the host Inter and is not comparable (7.1).
- **Boxes that do not count as a preceding sibling:**
  - `display: none` generates no box (CSS 2.1 §9.2.4).
  - A comment or a whitespace-only text node generates no box (css-text-3 §4.1.1).
  - An abspos / fixed box is out of flow (CSS 2.1 §9.3.1, css-position-3 §3) and holds no place in the block flow, so §4.1's "in-flow siblings" excludes it.
  - Floats and running elements are out of flow as well; the walk already says so (`:3753-3759`, `:3683`).
- **ADR-0010.** It governs where Rule 3 is relaxed. A relaxed card keeps `orphans` / `widows` where it can (Chrome's order, ADR-0010 item 2). `first_at_top` stays `cursor_y <= 0.0`: nothing precedes a first card.

## 3. Where it belongs, and the decisions

This is fragmenter behaviour, so the fix goes in the fork, in `crates/fulgur/src/pagination_layout.rs` only, inside `fragment_pagination_root`. No Blitz, Taffy or Parley change; no upstream PR or issue. In htmltopdf the work is the `Cargo.lock` bump, `docs/upstream.md`, tests and fixture 49.

**Decision 1: one shared notion, used in two places.** "A sibling precedes" is recorded explicitly instead of being inferred from `cursor_y > 0`. F213e-c uses it to take the Class A break. F213e-b uses its negation to drop Rule 4 and split instead of advancing.

**Corrected by step 1 (P10):** `precedes` must not use `cursor_y > 0.0`, as the call-site formula under F213e-b below did. Under body padding or border `cursor_y` is above 0 with no sibling, and Chrome splits (40 / 5) where `cursor_y > 0.0` would hold the card whole (main's blank page 1). Use the sibling record only: `precedes = sibling_on_page`, with `sibling_page` set on every placement path (so a card after a paragraph or any sized box still counts). `offset_above` stays a bound for F213e-c's move only.

```rust
// fragment_pagination_root: the page of the last in-flow sibling box placed, zero-height ones included.
let mut sibling_page: Option<u32> = None;
/// html-to-pdf F213e-c: a zero-height body child is a sibling box (css-break-3 §4.1): an element, not `display: none`
/// (the `clone_display().is_none()` idiom of `:1701-1705`), not a float, not abspos / fixed (skipped before the zero-height branch).
/// Corrected by step 1 (P7, 7.5): an anonymous block that holds an inline element counts even if the element is empty
/// (`<span></span>`: Chrome moves the card); a whitespace-only text node, a comment, `<script>` / `<style>` do not.
fn zero_height_child_is_sibling(child: &blitz_dom::Node) -> bool;
impl PageStrips {
    /// html-to-pdf F213e-c: the Class A break in front of a box of `size` at the top of `page` (`cursor <= 0`) is taken
    /// because a sibling precedes it there. Only where that gains room (page 0 under an offset, `offset_above`), only for
    /// a box that fits a fresh page (`size <= fresh() + 1.0`), measured against `chrome_bottom(page)` as F213e and F28
    /// (`:4455-4459`) do with the cursor at the top.
    pub(crate) fn sibling_break_moves(self, page: u32, cursor: f32, size: f32, sibling_on_page: bool) -> bool;
}
/// html-to-pdf F213e-b: `break-inside: avoid` on a body-level inline root holds where it is placed: its lines fit a fresh
/// page (`avoid_inside_holds`) and the box (`insets + lines_extent`) fits `room` or a break in front of it exists
/// (`precedes`). Otherwise Rule 4 is dropped (css-break-3 §4.4) and the box is routed like an `auto` one.
fn inline_avoid_holds_at(avoid_inside: bool, lines_extent: f32, insets: f32, room: f32, fresh: f32, precedes: bool) -> bool;
```

**F213e-c (PR A) changes three places.** `sibling_page` is set in the zero-height branch, for nodes that pass `zero_height_child_is_sibling`, and on every placement path. "On this page" means `sibling_page == Some(page_index)`.

1. **Plain advance (`:4534`):** add `|| strips.sibling_break_moves(page_index, cursor_y, child_h.max(keep_size), …)`. This moves the inline card.
2. **`keeps_whole_at`'s `above` (`:4397`, `:4464`):** `content_above || (sibling_on_page && offset_above(page))`. This keeps a block card whole, so the plain advance moves it.
3. **Paragraph move-whole (`:4182`):** the same `sibling_break_moves` term. Reason: ADR-0010's move-whole of a short paragraph needs this Class A break too. It is the line the finding named.

Left unchanged in PR A, each with a reason:

- **The forced-break guards (`:3770`, `:3863`):** forced breaks are not avoid; probe P9 decides whether to file anything.
- **`starts_with_relaxed_avoid` (`:4321`):** an avoid box taller than a page is F83b's.
- **The multicol move (`:3935`):** F30d.
- **The F28 wrapper move (`:4437`):** F213e's `offset_first` already moves monolithic first rows.
- **`keep_size` / `member`:** F215.
- **The `content_above` passed to `fragment_block_subtree`:** F83b Z1_234.
- **The nested walk's `content_above_child` (`:6382`):** body level only (§6).

**F213e-b (PR B) changes one decision.** At `:4013` compute `avoid_here = inline_avoid_holds_at(avoid_inside, lines_extent, insets, strips.chrome_bottom(page_index) - cursor_y, fresh, precedes)`, where `precedes = cursor_y > 0.0 || (sibling_on_page && offset_above)` (**corrected by step 1, see the note under Decision 1: `precedes = sibling_on_page`**). Then use `avoid_here` instead of `avoid_inside` in:

- the `line_metrics` choice;
- `inset_split_here` (`:4050`), so a bordered or padded card takes F30f-d's inset splitter and is sliced like Chrome's;
- the `body_paragraph_shape_ok` argument (`:4133`).

Clip cards (`clip_limit_here`, `:4024`) keep `avoid_inside`: an avoid card with `overflow` other than visible is a non-goal. The block path is untouched: cards of block children already split through `keeps_whole_at`.

**Decision 2: two PRs, F213e-c first.** AGENTS.md allows one PR only when one finding cannot be fixed without the other. F213e-c can be fixed alone. F213e-b cannot be fixed correctly without F213e-c's record: written with `cursor_y <= 0` it would also split the F213e-c shape (40 / 5), which is not Chrome's page 1 and would slip F213e-c's scope into its PR. With c first that dependency is gone, so the exception does not apply. Each PR gets its own byte gate and validator run. That reverses the pilot list's order; the c PR names this plan as the reason.

**F183c interaction.** These plans do not depend on each other:

- The cards are text only, so F183c's inline-box baseline does not change their line metrics.
- Fixture 49 has no inline-block or image.
- Both change the fork head, `Cargo.lock` and `docs/upstream.md`. Whichever merges second rebases and takes the other's binary as the byte-gate base.
- The F213e sweep's carrier cases move under F183c, not under this plan, so they are excluded from this plan's sweep criteria.

## 4. Acceptance

**Red tests to un-ignore:**

- PR A: `f213e_c5z_…` (0 / 45, AFTER on page 2, 2 pages).
- PR B: `f213e_c5_…` and `f213e_c6_…card` (40 / 5).

**New tests in `body_offset.rs`, Chrome m153 numbers from step 1:**

- **Avoid card of block children after a zero-height div** (`f213ec_blocks_after_zero_height_box_move_whole`): predicted 0 / 45, red on main. Without the div, `f213e_g_c5_…blocks` stays 40 / 5 (guard).
- **`<section>` cards:** the inline card in a section, first: **green on main, 40 / 5 as Chrome (P3)**, so a guard, not a red test; and the existing section-of-blocks guard. A zero-height div before the `<section>`, or inside it before the card, is **red on main and not covered by PR A's three call sites** (Chrome moves the card whole, 0 / 45; ours splits 40 / 5): file as F213e-d with those numbers, `#[ignore]`d.
- **Avoid cards that fit** (30 lines under 200 px, with and without a zero-height div): whole on page 1, 1 page. Guard.
- **`@page :first { margin-top: 60mm }`, F109** (this is a smaller page, not an offset, so `offset_above` is false):
  - a 30-line card that fits stays on page 1 (guard);
  - a 45-line card splits as Chrome on page 1; only page 1's count is asserted (guard);
  - a zero-height div before the 45-line card: **corrected by step 1 (P4): Chrome splits it 41 / 4, like ours**, so this is a green guard, not an ignored F109 test. No offset was consumed, so Chrome takes no break in front of the card. A 2 px div before it is the F109 case: Chrome moves the card whole (0 / 45), ours moves it and then splits it on page 2 (41 / 4 on pages 2 and 3), `#[ignore = "F109"]`.
- **Zero-height `<div style="counter-reset:polozka">`:** counts; whole card on page 2 (PR A).
- **`<div style="display:none">`:** does NOT count; 40 / 5 (PR B).
- **A comment, and a whitespace-only text node:** do NOT count; 40 / 5 (PR B).
- **`position:absolute` div:** does NOT count (§2); 40 / 5 (PR B). If Chrome counts it (P6), keep the spec reading and write ADR-0013 with the numbers, as ADR-0010 does.
- **Image of 1002 px after a zero-height div under `body{margin:8px}`:** stays on page 1, the C8 room (guard for `chrome_bottom`, PR A).
- **Edge cases:** room for 44 lines; a card ending in (`bottom(0)`, `chrome_bottom(0)`]. Chrome's numbers are pinned.

**Fork lib tests:**

- `page_strips_sibling_break_moves` (truth table: no offset, page 1 and later, taller than a page);
- `a_zero_height_sibling_counts_unless_hidden_or_out_of_flow`;
- `an_avoid_card_after_a_zero_height_box_moves_whole`;
- `a_first_avoid_inline_card_under_an_offset_splits_between_lines`;
- `inline_avoid_holds_at` truth table;
- one `render_smoke` case per PR.

With the new term forced to `false`, the red ones fail.

**Fixture: yes, `49-letterhead-avoid-notes-card`, in PR B.** A pre-printed letterhead with a header card whose addresses, payment details and a `nl2br` note are inline text is an invoice shape, and the pilot bar names it.

- **Layout:** `body { margin-top: 100mm }` in `<style>`. The first block is `div.break-inside-avoid.border.border-gray-300.p-4.text-sm.leading-5` of `<br>`-separated lines, tuned to between the 631 px of room and 1009 px in both engines. The items table follows.
- **Tailwind classes:** every class checked in the committed `tailwind.css`.
- **PNG:** page 1 shows the card's top lines from the letterhead line down, its frame sliced at the break, every line once. Page 2 shows the rest of the card and the table. Today the card runs off page 1.
- **Asserts (`fixtures.rs`):**
  - Chrome's page count;
  - every card line once, in order, with Chrome's per-page split;
  - first line within 1 pt of Chrome;
  - no yMax past the content bottom;
  - `no_fixture_has_content_that_is_painted_on_no_page` green (red before the fix).
- **Compare:** record the before number (on PR A's head) and set the threshold to after + 1 point.
- **No fixture for PR A:** it moves no fixture's pixels (byte gate below), and fixture 45 already covers the blank-page-1 rendering.

**Sweep against Chrome m153** (`out/scratch/f213eb/sweep.py`): offset source {body margin, collapsed card margin, html padding} × offset {8, 100, 200, 400} × card {45 `<br>` lines, the same with padding, wrapped paragraph, blocks, section + inline, section + blocks, flex, 30 lines} × before {nothing, zero-height `counter-reset` div, `display:none`, comment, abspos div, zero-height float, 2 px div}: 672 cases.

- Every changed case must be on Chrome's pages, none farther from them.
- Words lost and words below the content box must go to 0 for the card rows, F109 / F213b cases excepted.
- The rows "2 px before" and "block card, nothing before" are unchanged.

**Byte gates** (RAYON 1 and 4; any other change stops the work and a postmortem is written):

- **PR A:** fixtures 01-48 and the 7 corpus documents identical. Only 07, 26 and 45 have an offset, and none has a zero-height sibling before a box that overflows page 0 (step 1 confirms). In the fork, `cargo test -p fulgur` passes and the fulgur-vrt `.actual.pdf` sums are identical. The UA `body{margin:8px}` gives every fork test an offset, so re-bless only goldens Chrome confirms, and name each.
- **PR B:** the same, plus fixture 49 new.
- **Both:** the F213 1440-case sweep identical; `bench.py --check`.

## 5. Steps

1. **Measure, no code (≤ 1 day).**
   - Chrome m153 probes P1-P10:
     - P1-P2: the cards above;
     - P3: section;
     - P4: `:first`;
     - P5: `display:none` / comment / whitespace;
     - P6: abspos;
     - P7: zero-height float and empty-`<span>` anonymous block;
     - P8: the edge cases (44 lines, the 1 px band);
     - P9: a zero-height div then `break-before:page`;
     - P10: `body{padding-top:200px}` before a first card.
   - A fork unit test confirms the §1 path. *Done instead by reading the fork source plus the probe evidence (7.9); the fork was not built.*
   - Record in §7. If P6 or P10 contradicts §2, re-plan that line only. *Done: P6 agrees with §2; P10 contradicts the `precedes` formula, corrected above and in 7.8.*
2. **PR A red (≤ ½ day).** Our tests and the fork lib tests; red as listed.
3. **PR A fork (≤ 1 day).** `sibling_page`, `zero_height_child_is_sibling`, `sibling_break_moves` and the three call sites; lib tests, clippy, fmt, `cargo test -p fulgur`, fulgur-vrt.
4. **PR A gates and merge (≤ 1 day).** Bump, byte gates, sweep (c rows), bench, `vrt-validator`, findings and `docs/upstream.md`. No `pdf-reviewer`: layout only. Report, then continue with F213e-b (the owner asked for continuous work).
5. **PR B red and fixture 49 (≤ 1 day).** `render.mjs` reference, README, red tests.
6. **PR B fork (≤ 1 day).** `inline_avoid_holds_at` and the `avoid_here` routing; lib tests; fork suite.
7. **PR B gates (≤ 1 day).** Bump, byte gates, sweep (all rows), `render-fixtures.sh`, `compare.py`, look at 49's side-by-side, `vrt-validator`, findings, §7.

## 6. Risks and non-goals

- **Blank page 1.** A zero-height element under a letterhead now sends a card to page 2, leaving page 1 with only the letterhead. That is Chrome's output (c5z). The `offset_above` and `fresh() + 1` bounds stop it in documents without an offset, where moving gains nothing.
- **The UA 8 px margin in fork tests** gives every fork test an offset. A gcpm test with a reset div before a spacer that fills a page could move; re-bless only goldens Chrome confirms.
- **Rule 3 in the relaxed split.** `first_at_top` relaxes widows at a page top, so with room for 44 lines the split may be 44 / 1. **P8 pins Chrome: 43 / 2 for the inline-text card** (widows 2 kept, as for a plain paragraph, which ours already splits 43 / 2). A card of block children splits 44 / 1 (widows do not apply across blocks), which ours also does. The inset-splitter route of PR B must give 43 / 2; 44 / 1 would be wrong.
- **Non-goals, each filed with Chrome's numbers if a probe is red:**
  - the nested walk, e.g. a zero-height sibling inside a `<section>` (F213e-d);
  - forced breaks after a zero-height sibling (P9);
  - body padding before a first card (P10);
  - avoid clip cards;
  - `@page :first` heights (F109);
  - keep chains (F215);
  - boxes taller than a page (F213b / F29b);
  - `vertical-align` (F183c).

## 7. Measurements (step 1, 2026-11-09)

Step 1 ran no cargo command and built nothing. Probes: main binary and Chrome m153; the fork was only read.

### 7.1 Method

- **Ours:** the main binary (`0cbe50f`, fork pinned in `Cargo.lock` at `5215c775`; the fork head `f2bfc96f` (F183c) differs in `pagination_layout.rs` by one comment), `RAYON_NUM_THREADS=1`, the five bundled Inter TTFs from `fixtures/fonts`, `--image red60.png=…`. A4, 15 mm margins: content box 1009.13 px = 756.85 pt, content bottom 799.37 pt, page 841.89 pt.
- **Chrome:** Playwright with `/opt/pw-browsers/chromium` (153.0.8010.12), `emulateMedia print`, `preferCSSPageSize`, `printBackground`.
  - **The probe copy given to Chrome carries `@font-face` rules that point at the same five bundled TTFs.** The host has its own Inter (OTF) with other glyph-box metrics, which moved every first line by 2.6 pt (Chrome 41.7 pt, ours 44.3 pt for a card at the content top). With the bundled face Chrome's first line is 44.6 pt, ours 44.3 pt, and the 0.3 pt is the same everywhere. The 41.7 pt in section 2, in the F213e tests and in `body_offset.rs` is the host-font number; the plan's "first line within 1 pt" assertion for fixture 49 is valid because the fixture is rendered with the bundled faces.
- **Document:** `<style>` = `@page{size:A4;margin:15mm} body{font:14px/20px Inter;margin:0} p{margin:0}` plus the case's CSS (`body{margin-top:200px}` unless stated). Card = `<div style="break-inside:avoid;border:1px solid #000">` of `Line0001<br>…Line0045` (45 lines, 900 px + 2 px border); blocks card = the same with one `<p>Line00NN</p>` per line; `AFTER` = `<p>AFTER</p>` after the card; `Z` = `<div style="height:0"></div>`.
- **Reading:** `pdftotext -bbox` (page and yMin of each `Line…` / `AFTER` word) and `pdfimages -list` for images. Content painted off the sheet: `qpdf --qdf`, MediaBox rewritten to `[0 -3000 w h]`, `pdftotext -bbox` again (F213 7.2).
- **Notation:** `a/b` = number of `Line` words on pages 1 / 2; `0/45` = page 1 empty, the card whole on page 2. "hang" = main's result for the F213e-b shape: lines 1-43 on page 1 (42-43 below the content bottom, last yMax 837.1 pt), lines 44-45 off the sheet (found at 867.1 pt only in the widened read-back, so lost), `AFTER` on page 2 at 43.6 pt, and the warning "content extends 22.6 pt past the bottom page edge". "red" = ours differs from Chrome in the pages; "green" = same pages.

### 7.2 P1, P2: the cards and the zero-height sibling (offset 200 px)

| Shape | Chrome | main |
|---|---|---|
| P1: card first (C5) | 40/5, L1 at 194.6 pt, page 2 L41 at 43.8, `AFTER` p2 119.6 | hang: red |
| P1: card first, margin 200 px collapsed through body (C6) | 40/5, same y | hang: red |
| P2: `Z`, card (C5z) | 0/45, L1 at 44.6, `AFTER` p2 720.3 | hang: red |
| `Z Z` card; `<p></p>`; `<div style="counter-reset:polozka">`; zero-height `position:relative` / `sticky`; `clear:both` zero div; zero-height `display:flex`; empty `<table style="height:0">`; block `<img>` of height 0 | 0/45 | hang: red |
| zero div with `margin-bottom:10px`, card | 0/45 | hang: red |
| `<div style="height:0.4px">`, card | 0/45 | hang: red (Taffy rounds 0.4 px to 0, so the zero-height branch is taken; 0.5 px rounds to 1) |
| 0.5 px, 1 px, 2 px div, card | 0/45 | 0/45: green (`cursor_y > 0`) |
| blocks card first | 40/5 | 40/5: green |
| `Z`, blocks card | 0/45 | 40/5: **red** (the block-children twin of C5z; the plan predicted it) |
| 0.4 px, blocks card | 0/45 | 40/5: red; 0.5 px and larger: green |
| `Z` + `margin-bottom:10px`, blocks card | 0/45 | 40/5: red |
| card of 30 lines, first, or after `Z` | 1 page, whole | same: green |
| `<p>Head</p>`, then a 40-line card (inline or blocks) | Head p1, card p2 | same: green |
| `body{margin:8px}`: 45-line card first or after `Z` | 1 page | same: green |
| `body{margin:8px}`, `Z`, block image of 1000, 1001, 1002 px | stays p1 | same: green |
| the same with 1003 px (and without `Z`) | moves, p1 empty, 3 pages | same: green (the C8 room guard holds) |

Predictions of section 1 and 4 hold: P1 40/5, P2 0/45, blocks-after-`Z` red on main.

### 7.3 P3: `<section>`

| Shape | Chrome | main |
|---|---|---|
| `<section>` of the inline card, first | 40/5 | 40/5: **green** |
| `<section>` of the blocks card, first | 40/5 | 40/5: green |
| bare wrapper `<div>` of the inline card, first | 40/5 | 40/5: green |
| `Z`, then `<section>` of the inline card | 0/45 | 40/5: red |
| `Z`, then `<section>` of the blocks card | 0/45 | 40/5: red |
| `<section>` that holds `Z` and then the inline or the blocks card | 0/45 | 40/5: red |
| wrapper `<div style="border:1px solid #f00">` holding the inline card | 40/5 | **page 1 empty, 0/45: red, the opposite direction (the card is moved)** |

The inline card in a section is not a red test (section 4 expected it might be). The four shapes with a zero-height sibling around a `<section>` are red and **not reached by the three call sites of PR A** (`content_above` passed to `fragment_block_subtree` and the nested walk are on the "left unchanged" list). They are F213e-d: file them with these numbers.

### 7.4 P4: `@page :first { margin-top: 60mm }` (page 1 holds 839 px, 41 lines of a card)

| Shape | Chrome | main |
|---|---|---|
| 30-line card | whole on p1, L1 at 172.1 | same: green |
| 45-line card | 41/4 | 41/4: green |
| **`Z`, 45-line card** | **41/4** (not moved) | 41/4: **green** |
| `Z`, 30-line card | whole on p1 | same |
| 2 px div, 45-line card | 0/45 (p1 empty, card whole on p2) | p1 empty, then 41 on p2 and 4 on p3: red (F109: page 2 is given page 1's height) |

**This contradicts section 4's `@page :first` bullet** (it predicted Chrome moves the card after `Z`; it does not). Chrome takes no break in front of a box when nothing has been consumed above it in the fragmentainer, which is the `offset_above` bound the plan already has; here page 1 is merely smaller. Corrected in section 4. Without `@page :first`: `Z` before a 45-line card at no offset stays on page 1 in both (also in 7.2: `body{margin:8px}`).

### 7.5 P5, P6, P7: what counts as a preceding sibling (offset 200 px, 45-line card, `Z`-like element before it)

**Chrome moves the card whole (0/45): the element counts.** Ours: hang unless noted.

- a zero-height `<div>`, `<p></p>`, `counter-reset` div, `position:relative` and `position:sticky` zero-height divs, `clear:both` zero div, zero-height flex container, empty table, block `<img>` of height 0;
- **an empty inline element: `<span></span>`, or ` <span> </span> ` with spaces. This contradicts section 3's "not an anonymous block without line boxes".** With the blocks card the same: Chrome 0/45, ours 40/5 (red);
- `<br>`, `&nbsp;` as bare text in the body, `<p>&nbsp;</p>`, an inline `<img style="width:0;height:0">`: they make a line box, ours moves the card too (0/45, green).

**Chrome splits the card in place (40/5): the element does not count.** Ours: hang (this is the F213e-b shape, so these are red until PR B).

- `<div style="display:none">`, `<div hidden>`, an HTML comment, whitespace-only text (`\n   \n\t`), `<script>` and `<style>` in the body;
- `<div style="display:none">` with the blocks card: 40/5, ours 40/5 (green);
- **abspos and fixed (P6): `position:absolute` with `height:0`, with a 10x10 box, and `position:fixed` do not count.** Section 2 and the plan's abspos test stand; no ADR-0013 is needed. An abspos div followed by `Z` moves (the `Z` counts). The blocks card after an abspos div: 40/5 on both;
- **floats (P7): a float of height 0 does not count** (Chrome 40/5; with the blocks card both 40/5). A float of 10 px: Chrome 40/5, **ours moves the card whole** (page 1 empty): red in the other direction, out of scope (F202 is floats), file with the numbers;
- `display:none` then `Z` then the card: 0/45 in Chrome (the `Z` counts), hang in ours.

### 7.6 P8: edge cases (45-line card, `body{margin-top:M}`)

- **Room for 44 lines** (M = 126 to 131, 881 px of room at M = 128): Chrome **43/2** at every M (widows 2 kept); ours hangs (45 on page 1). A plain 45-line paragraph at M = 126 and 128: Chrome 43/2, ours 43/2 (green). A **blocks** card at M = 128: Chrome **44/1**, ours 44/1 (green). With `Z` before the card (M = 126, 128, 130): Chrome 0/45, ours hangs.
- **The 1 px band.** 30-line card (602 px) with `body{margin-top:M}` so that it ends at M + 602:
  - M = 406, 407, 407.5, 408 (ends at 1008 to 1010 px): Chrome whole on page 1, with or without `Z`; ours whole (green).
  - M = 408.5 (ends at 1010.5) and above: Chrome splits 28/2 (M = 408.5 to 430), 27/3 (450), 26/4 (480), 25/5 (500), 24/6 (510 to 525), 23/7 (540), 22/8 (560); with `Z` before it, 0/30. Ours keeps all 30 lines on page 1 for every M from 408.5 to 560 (hang): from M = 480 the lines past the page edge are lost (29, 28, 27, 26, 25 lines on the sheet at M = 480, 500, 510 to 525, 540, 560), with the warning.
  - So Chrome's room is 1010 px, as `chrome_bottom` has it, and the card must be held against `chrome_bottom(page) - cursor`, not `bottom(page) - cursor`: a card ending within (1009.13, 1010] stays whole (a green guard for PR B, as planned).
- A card with 16 px of padding (`pad`), first: Chrome 39/6, L1 at 206.6; ours hangs (42 on page 1). With `Z`: Chrome 0/45, ours hangs. A wrapped card (44 text rows of 200 px width): first 40/4, with 16 px padding 39/5, after `Z` whole on page 2 (44 rows); ours hangs both ways.
- A 60-row card taller than a page (the first wrapped probe, 1200 px): after `Z` Chrome starts it on page 2 and splits it there; ours splits it on page 1. That is F83b / F213b, out of scope.

### 7.7 P9: `Z` and then a forced break

| Shape (offset 200 px unless stated) | Chrome | main |
|---|---|---|
| `<p style="break-before:page">AFTER</p>` first | no break: one page, AFTER at 193.8 | same: green |
| `Z`, then the `break-before:page` paragraph | **p1 empty, content on p2** | no break, everything on p1: red |
| 2 px div, then the same | p1 empty, content on p2 | same: green |
| `Z`, then the same, **no offset** (`body{margin-top:0}`) | **p1 empty, content on p2** | no break, one page: red |
| `Z` then a `div` with `break-before:page` holding a 3-line card | p1 empty | no break: red |
| `<div style="height:0;break-after:page">`, card | p1 empty, card on p2 | same: green |

A zero-height sibling is a real Class A break point for a forced break as well, and the guards `zero_force_break && cursor_y > 0.0` (`:3770`) and `(explicit_break_before || page_name_changed) && emitted > 0 && cursor_y > 0.0` (`:3862`) read it as "nothing precedes". Same blindness as F213e-c, but forced and offset-independent. PR A leaves them unchanged as planned; **file it** (F213e-e, low-medium) with the numbers above.

### 7.8 P10: body padding / border before a first card

| Shape (45-line card first, AFTER after it) | Chrome | main |
|---|---|---|
| `body{padding-top:200px}` | **40/5** | p1 empty, 0/45: red |
| `body{margin-top:100px;padding-top:100px}` | 40/5 | 0/45: red |
| `body{margin-top:199px;padding-top:1px}` | 40/5 | 0/45: red |
| `body{border-top:200px solid #ccc}` | 40/5 | 0/45: red |
| `html{padding-top:200px}`; `html{border-top:200px solid #ccc}` | 40/5 | hang: red (F213e-b, `offset_above`) |
| `<div style="padding-top:200px">` wrapper around the card | 40/5 | 0/45: red |
| `Z` after the padding, then the card | 0/45 | 0/45: green |
| blocks card under `body{padding-top:200px}` | 40/5 | 40/5: green |
| 30-line card under `body{padding-top:200px}` | whole on p1 | same: green |

**Contradiction with the plan.** Chrome does not take a break in front of a card that is the first child of a padded or bordered body, so `cursor_y > 0.0` produced by that padding does not mean "something precedes" for a breakable box (it does for an image: `f213e_g_body_padding_and_border_already_count_as_content_above` stands). Main moves these cards whole and shows a blank page 1: a mismatch already present, in the other direction. PR B's `precedes` must therefore be the sibling record only (corrected in section 3), which also makes the padding rows above 40/5. If that proves hard, these rows go on the findings queue (F213e-f, medium: blank first page) instead of being fixed; they must not regress either way.

### 7.9 The code path of section 1 (fork source read, `git show HEAD:crates/fulgur/src/pagination_layout.rs`, head `f2bfc96f`)

The path in section 1 is right; its line numbers hold within 5 at that head (`avoid_inside_holds` fn `:1854`; the empty `line_metrics` `:4013-4016`; `inset_split_here` `:4046`; the paragraph move `:4182-4184`, `cursor_y > 0.0 && …`; `content_above` `:4320`; the plain advance `:4532-4538`; the zero-height branch `:3723-3800`). What the code and the probes show together:

- The card is an inline root with `break-inside: avoid`. `avoid_inside_holds(true, 900, fresh)` is true, `line_metrics` is empty, so the paragraph split and the `para_total_h` advance are skipped. The plain advance moves only when `cursor_y > 0.0 && cursor_y + child_h … > bottom` or `first_monolithic_moves`; `first_monolithic_moves` needs `monolithic_box(…)`, which a card of text is not. Probe evidence: the card is placed whole at `cursor_y = 0` on page 0 (hang: widened read-back, all 45 lines painted, 44-45 off the sheet); a 1 px or 2 px div before it (`cursor_y > 0`) moves it whole, a 0 px or 0.4 px one does not.
- The zero-height branch (`child_h <= 0.0`) `continue`s without moving the cursor. **Taffy rounds layout sizes to whole px**, so `height:0.4px` takes it too (7.2); `0.5px` does not. A comment node and a `display:none` element arrive in it as well (the comment has no `element_data`, the hidden element has `clone_display().is_none()`), so `zero_height_child_is_sibling` must test both. Abspos / fixed children are skipped before it (`:3662-3668`) and so are running elements (`:3683`); a float of height 0 is handled inside it (`zero_is_float`, `:3760`, kept out of `prev_used_page`). That is Chrome's behaviour for all three (7.5).
- `content_above = cursor_y - gap > 0.5` (`:4320`) is why a block card after a zero-height sibling is entered (`keeps_whole_at(…, false)`) and split 40/5: the gate sees "nothing above".
- A box after the zero-height branch has `child_h > 0`, so it is placed by the normal path and `cursor_y` advances; this is why "sibling placed" must be recorded in both places, as the plan says, and why a sibling of height 0.5 px or more already works.
- A whitespace-only text node is skipped before everything (`:3646-3652`), consistent with Chrome (7.5). An anonymous block around an empty `<span>` is a child with height 0 and goes through the zero-height branch; whether Blitz gives it `element_data` was **not** checked (no fork test): `zero_height_child_is_sibling` must not rely on `element_data().is_some()` alone, or the `<span></span>` rows of 7.5 stay red.

### 7.10 Fixtures 07, 26, 45

Body-level children listed from the three `index.html` files with a small HTML parser (that only 07, 26 and 45 have an offset is the plan's statement, not rechecked here). All of this is a read of the source, not a measurement:

- **07** (`body` without margin, `h1.mt-8` first): `h1`, `p`, `section`, `table`, `p`, `p`, all with height. No zero-height sibling, no hidden or abspos child.
- **26** (`@page margin 0`, `body{margin:15mm}`): a comment, an abspos stamp `div` (height from its text, not zero, abspos so not a sibling), then `header`, `section`, `table`, `table`, `section`. The first in-flow box is the `header` (a title and two short lines), which is far smaller than page 0's room. No zero-height sibling before a box that overflows page 0.
- **45** (`body{margin-top:70mm}`): `main.inline-block.break-inside-avoid` (the card, first), then `p`. Nothing precedes the card, so PR A changes nothing; PR B's rule (split a first avoid inline card) does not apply either: the card's content is block children (`header`, `section`, `table`), which already go through `keeps_whole_at`. The byte gate (steps 4 and 7) is the proof.

### 7.11 What changes in the plan

- `precedes = sibling_on_page` only (7.8). Section 3 corrected.
- `zero_height_child_is_sibling` counts an anonymous block with an inline element, empty or not (7.5). Section 3 corrected.
- Section 4: the `@page :first` bullet (7.4) and the `<section>` bullet (7.3) corrected; the 41.7 pt figure is 44.6 pt (7.1); the Rule 3 risk is settled at 43/2 (7.6).
- Tests to add or pin with these Chrome numbers (names are proposals):
  - PR A: the 0.4 px sibling (0/45), the `<span></span>` sibling (0/45), the `margin-bottom:10px` zero div (0/45), the blocks card after `Z` (0/45), the abspos / fixed / float-0 / `display:none` / comment / whitespace rows as guards for "does not count" (40/5 after PR B; before it they hang, so they belong to PR B);
  - PR B: room for 44 lines gives 43/2; the 1 px band guard (M = 407.5 and 408 whole, 408.5 gives 28/2); the padded card 39/6; the padding rows of 7.8 only if they are fixed.
- Files for the findings queue (Chrome's numbers are above): F213e-d (zero-height sibling around a `<section>`, 7.3), F213e-e (forced break after a zero-height sibling, 7.7), F213e-f (blank page 1 for a first card under body padding or border or in a bordered wrapper, 7.3 and 7.8), the 10 px float and the `display:flex` first card moving whole where Chrome splits (7.5, 7.2 `flex`), `@page :first` with a 2 px sibling (F109, 7.4), and a 49-line card (982 px) under `body{margin:8px}` whose `AFTER` moves to page 2 where Chrome keeps it on page 1 (Chrome's 1010 px room applies to a paragraph too; F30f-b has it for clip boxes only).

### 7.12 Not verified

- Anything about the fork's behaviour beyond reading it: no fork test, no build, no `render_smoke`.
- The spec wording of section 2 (css-spec-researcher not run).
- Whether Blitz gives the anonymous block of an empty `<span>` an `element_data` (7.9).
- Page size A4 only; the offsets 100 and 400, the other card kinds of the 672-case sweep and `html` padding as a source (except 7.8) were not probed.
- The sweep, the byte gates and the fixture 49 layout are later steps.
- Chrome's numbers are one run each on m153; Chrome's own order of widows / orphans was read from the results (43/2), not from its source.
