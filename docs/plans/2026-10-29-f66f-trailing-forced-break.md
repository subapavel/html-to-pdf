# F66f: a forced break at the end of a block makes an empty trailing line

Date: 2026-10-29. Findings: F66f (medium) and F37d (low) in `2026-10-15-m0-findings.md`, same cause. Fork: `subapavel/blitz`
`htmltopdf/blitz-dom-0.2.4` (`layout/inline.rs`, `break_lines`, the F32a function). Parley stays unchanged.

## 1. Problem

`<div>a<br>b<br></div>`, `<div style="white-space:pre">a\nb\n</div>` and `<div style="white-space:pre-line">\n  Firma\n  Ulice 1\n</div>`
are one line (20 px = 15 pt) taller than in Chrome. Parley's `BreakLines::break_next` commits the line that holds the newline
cluster (`BreakReason::Explicit`), starts a new line, and on the next call, with no item left, commits that new line empty
(`BreakReason::None`, `greedy.rs` end of `break_next`). Blitz's `break_lines` keeps it, so `layout.height()` (and Taffy's
measured height, which comes from the same function in `ComputeSize` mode and in the real pass) has one line too many.

Consequences seen so far: every address block or card ending in `<br>` or a newline; padded body-level cards ending in `<br>`
are 15 pt too low after F30f-h (`out/scratch/f30fhv/g1/z06_br_trailing.html` 133.6 against Chrome 118.8); at an exact fit the
empty line pushes the next block to the next page (`f30fdv/a/exact_n44_pb22`, `exact_n45_pb20`, `exact_n45_pb21`); the widows
rule counts the empty line (`exact_n47_pb21`). F37d is the body-level case (`Line1<br>…<br>` directly in `body`), and F183's
strut made `<img><br>` 8 px worse.

## 2. Spec

- CSS 2.1 §9.4.2 (inline formatting context): line boxes are made by the content; a line box that contains no text, no
  preserved white space, no inline element with non-zero margins, padding or borders and no other in-flow content (such as
  images, inline blocks or inline tables), and does not end with a preserved newline, is treated as zero-height. The line that
  ends with the preserved newline is the line of the `<br>`; what follows it is not a line unless something is on it.
- CSS Text 3 §4.1.3 / §4.1.1: collapsible spaces at the start and end of a line are removed; spaces after a segment break
  (`pre-line`) are removed. Blitz's `whitespace.rs` already does this, so a trailing `<br> ` or `\n   ` under `pre-line` reaches
  `break_lines` as the bare newline. Under `pre` / `pre-wrap` the spaces are preserved and make a real line.
- CSS Text 3 §3 (`white-space`), HTML rendering §15.3.7: `<br>` is a forced line break (U+000A).

Which layer is right: Parley's `break_all_lines` has no notion of a block; a trailing newline giving a last empty line is what a
text editor (textarea) shows, and it is the caller's decision whether the line exists in flow layout. `break_lines` in Blitz is
already that place (F32a, trailing line after an overflowing inline box), so the fix is there and Parley needs no change.

## 3. Measurements (Chrome m141, A4, 15 mm, Inter 14px/20px, `out/scratch/f66f/{gen.py,run.sh}`, git-ignored)

AFTER's top (pt) for `<div>SHAPE</div><p style="margin:0">AFTER</p>` (and the same in a `border:1px; padding:9px` card, +15 pt).
First line is 58.8 in Chrome; every line adds 15. `main` = binary of 8c66efb.

| shape | Chrome | main |
|---|---|---|
| `a<br>b<br>` | 73.8 (2 lines) | 88.6 |
| `pre-line` `\n    Firma\n    Ulice 1\n` | 88.8 (3: the leading newline is a line) | 103.6 |
| `pre` `a\nb\n` | 73.8 | 88.6 |
| `a<br><br>` | 73.8 (2: the first `<br>` ends line 1, the second is line 2) | 88.6 |
| `<br>` only | 58.8 (1 line) | 73.6 |
| `a<br>b<br><br><br>` | 103.8 (4) | 118.6 |
| `<br><br><br>` | 88.8 (3) | 103.6 |
| `pre` `a\n\nb` (empty line in the middle) | 88.8 | 88.6 (right already) |
| `pre` `a\n\n` | 73.8 (2) | 88.6 |
| `pre` `a\n  ` (spaces after the newline) | 73.8 (the spaces make a line) | 73.6 (right) |
| `pre-wrap` `a\n   ` | 73.8 | 73.6 (right) |
| `pre-line` `a\n   ` | 58.8 (the spaces vanish, no line) | 73.6 |
| `pre-wrap` `a   \n` | 58.8 | 73.6 |
| `a<br> ` / `a<br>\n  ` (source white space) | 58.8 | 73.6 |
| `<b>x<br></b>`, `<span>a<br></span><span></span>`, `a<br><!-- c -->` | 58.8 | 73.6 |
| `pre` `\n` only | 58.8 (1 line) | 73.6 |
| `pre-line` `\nx` | 73.8 | 73.6 (right) |
| `a<br>&nbsp;` | 73.8 (NBSP is content) | 73.6 (right) |
| `pre` `a\r\nb\r\n` | 73.8 | 88.6 |
| `<li>a<br></li>`, table cell `a<br>`, `nowrap`, `inline-block` holding `a<br>`, RTL, centred, flex item, narrow wrapping text then `<br>` | one line less than main in every case | |
| `Line1<br>Line2<br><div>AFTER</div>` (body-level, F37d) | 73.8 | 88.6 |
| `a<br><span style="display:inline-block;width:20px;height:20px">` | 77.6 | 73.6 (4 pt: the inline-block strut, F183b; not this finding) |
| `a<br><inline-block width:0;height:30px>`, `a<br><svg height=30>` | 85.1 | 81.1 / 84.8 (same, F183b; the box is content, the line stays) |

So the rule is: the line after a forced break is dropped when nothing is on it. Anything on it (preserved spaces, NBSP, a
zero-width inline box, an image) keeps it.

## 4. Fix

`break_lines(layout, text, width, nowrap)` (Blitz fork): after the loop, the final `break_next` result is a line of zero
advance, the breaker is done, and the text ends in a newline cluster (`\n` or `\r`, Parley's `Whitespace::Newline`) and no
inline box is placed at the end of the text: take it back with `BreakLines::revert`, as the F32a branch already does for the
overflow case. Used by both the measuring clone (`ComputeSize`) and the real pass, so Taffy's measured height shrinks with the
painted one. A block that holds only `<br>` keeps its single line: the line with the newline is line 1, the empty one is line 2.

Not done: nothing in fulgur or htmltopdf changes (they take line metrics from the finished layout).

## 5. Acceptance

- `crates/htmltopdf/tests/trailing_break.rs` (red before): the AFTER positions of §3 within 1 pt of Chrome (`a<br>b<br>`,
  the `pre-line` address block, `pre`, `<br><br>` keeps two lines, `<br>` alone keeps one, an empty middle line stays, preserved
  trailing spaces stay, `<b>x<br></b>`, a list item, a table cell, a bordered card, a body-level run), plus the exact fit
  that used to jump to page 2.
- Blitz fork lib test in `layout/inline.rs` (`trailing_forced_break`): line counts for the shapes above.
- `fixtures/invoices/` and `bench/corpus/`: the byte gate against 8c66efb lists what moves; each moved fixture must move toward
  Chrome (compare.py numbers in §7); thresholds lowered per AGENTS.md.
- The F30f-h sweeps (`f30fhv`, `f30fh`, `f30fdv`): `AFTER` of the trailing-`<br>` cards within 1 pt of Chrome, the three
  exact-fit page-count differences gone.

## 6. Risks

- Fixtures with a `<br>` that ends a block or sits alone between blocks: 27 (`<dd>…</dd><br>` in a `dl`: a bare `<br>` in an
  anonymous block, one line in Chrome, two lines here today), 32 (card text); a grep for `<br>` before a closing tag finds
  none, the byte gate is the authority. `pre-line` is used only in 22 (`.note`, no trailing newline).
- A text that ends in a newline cluster but whose last line is not the trailing empty one cannot occur: the check needs the
  breaker done and advance 0, and the only way to get there with text ending in a newline is the empty tail line.
- The fork's VRT goldens (`fulgur-vrt`) may hold a trailing-`<br>` case; any that moves must be one, Chrome-confirmed, and is
  recorded in `docs/upstream.md` like `basic/box-shadow` (not re-blessed on this host).
- Performance: one comparison per inline layout; no measurable cost.

## 7. Evidence

### What was built

- Blitz fork `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`, commit `6c2da16d` (on `63628c03`): `break_lines(layout, text, width, nowrap)`
  takes back the final empty line when `breaker.is_done()`, the advance is 0, the text ends in `\n` / `\r` and no inline box
  sits at the end of the text (one more condition on the F32a branch). Both callers pass `inline_layout.text`; the measuring
  clone and the real pass use the same function, so Taffy's height shrinks with the painted lines (checked: `div.final_layout`
  height is `20 * lines` in the fork tests). Seven lib tests `trailing_forced_break_tests` (five red before the change). Parley
  and fulgur are unchanged; htmltopdf only locks the new Blitz commit (`cargo update -p blitz-dom -p blitz-html`; cargo also
  tried to move `skrifa 0.44.0 -> 0.42.1` under `swash`; that line was put back by hand, the lock diff is the two Blitz
  lines).
- `crates/htmltopdf/tests/trailing_break.rs`: ten tests, 30 shapes (plain, and inside the F30f-h card), the three exact-fit cards,
  the widows split. Eight red before, ten green after; the two that passed before are the "what stays" guards.

### Chrome against the fix (`out/scratch/f66f/a`, 37 shapes x plain / `_box` / `_nest`)

Every shape of §3 now gives Chrome's AFTER within 0.3 pt (examples: `a<br>b<br>` 73.6 / Chrome 73.8, `pre-line` address block
88.6 / 88.8, `a<br><br>` 73.6 / 73.8, `<br>` alone 58.6 / 58.8, `pre` `a\n\nb` and `pre` `a\n  ` unchanged). The only shapes still
off are the two inline-block-after-`<br>` ones (4 pt, F183b: `s17`, `s28`), unchanged from main. Side-by-side main | fixed |
Chrome (`out/scratch/f66f/v/sbs.png`): the pre-line address card and the four-line card lose the empty line and end where
Chrome's do; the `<br>` followed by an inline-block keeps its second line.

### Gates

| gate | result |
|---|---|
| Blitz `cargo test -p blitz-dom --lib` | 144 passed (137 on the base); with `parallel-construct` 146 passed. Two `log`-capturing tests failed once in a run that shared the machine with another cargo job and never again (3 reruns green); not related to layout |
| fulgur (forks patched in) `cargo test -p fulgur --lib` | 2420 passed |
| fulgur integration binaries, one at a time | the lib and all 43 integration binaries ok (`render_smoke` 326 passed) |
| fulgur-vrt | 35 of 64 differ on this host, as on the F30f-h base; the whole diff directory (70 files) is byte-identical to the base's: no golden moves, `basic/box-shadow` stays as recorded |
| html-to-pdf `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` | clean |
| byte gate vs `htmltopdf-main-f30fh`, 1 and 4 threads | all 39 documents (fixtures 01-32, 7 bench corpus) byte-identical; 1 thread and 4 threads identical to each other |
| `render-fixtures.sh` + `compare.py` | all 32 ok; every number equal to main's, so no threshold is lowered and none raised |

No fixture or corpus document ends a block in a `<br>` or a preserved newline (the grep of §6 found none; fixture 27's
`<dd>…</dd><br>` is inline-level inside the `dl` and not trailing), which is why the fixtures do not move. The change is shown by
`tests/trailing_break.rs` and the sweeps, and by fixture 33 added afterwards (below).

### Fixture 33 (`33-address-trailing-break`, added at the coordinator's request, AGENTS.md "fixtures are the spec")

A one-page Czech invoice: `pre-line` supplier address with a trailing newline, customer address of `<br>`-terminated lines, a padded
note card ending in `<br>`, the payment block below (`whitespace-pre-line` is not in the compiled Tailwind sheet, so inline style).
`compare.py`: 7.8 % on main's binary -> 3.8 % now, threshold 4.8 %. y of "Celkem" / "Poznámka" / "PLATEBNÍ" (pt): main 340.7 / 391.6 /
556.6, now 325.7 / 376.6 / 526.6, Chrome 325.9 / 376.8 / 526.8. Side-by-side: on main the party grid and the card each carry one
blank line, the items table and everything below are shifted down (the payment block 30 pt); now the page is Chrome's line for line,
the diff heat map shows only the usual text-drift glyph edges. Test: `fixtures.rs` `address_trailing_break_invoice_is_laid_out_like_chrome`
(three words within 1 pt of Chrome, one page). Byte gate: fixtures 01-32 and the corpus (39 documents) identical to main at 1 and 4
threads; only 33 differs. The test was run green on the branch; it was not run on main's binary (the numbers above are main's).

### Sweeps (F30f-h's `f30fbv*`, `f30fd`, `f30fdv`, `f30fh`, `f30fhv`: 3381 cases, `out/scratch/f66f/sweep.{py,jsonl}`, main = `htmltopdf-main-f30fh`)

3345 byte-identical, 36 changed, all trailing-`<br>` documents. Of the 36, AFTER within 1 pt of Chrome: 10 on main -> 31 now;
page counts different from Chrome's: 5 -> 1.

- `f30fhv/g1/z01`, `z02`, `z06` (padded card ending in `<br>`): AFTER 118.6 -> 103.6, 88.6 -> 73.6, 133.6 -> 118.6 (Chrome 103.8,
  73.8, 118.8). `f30fh/z/z1`-`z3`: 148.6 / 133.6 / 148.6 -> 133.6 / 118.6 / 133.6 (Chrome 133.8 / 118.8 / 133.8).
- Exact fits: `exact_n44_pb22`, `exact_n45_pb20`, `exact_n45_pb21`, `un_n47` back on one page like Chrome (were two);
  `exact_n44_pb20/21` AFTER 784.6 -> 768.8, 785.3 -> 769.6 (Chrome 769.1, 769.8); `exact_n47_pb20-22`, `exact_n48_*`, `pd_n48`,
  `pd_n49`, `pdt_n48`, `pd_n50/51`, `pdt_n49-51`, `un_n50/51`, `exact_n46_pb20/21`: the split is now Chrome's (45 / 3 lines
  etc.), `exact_n47_pb21` is the finding's widows case.
- `br.html` (f30fdv, 3 pages): AFTER on page 3 459.1 -> 446.3 (Chrome 445.8).

### Moved away from Chrome, or still off: pre-existing, exposed, filed as F30f-l

Five f30fdv cases are not Chrome's: `exact_n45_pb22`, `exact_n46_pb22`, `pd_n47`, `pdt_n47`, `un_n49`. One of them, `un_n49`,
moved away (AFTER 75.8 on main, Chrome 76.1, now 44.3). Each renders the same bytes as **main with the trailing `<br>`
removed from the source** (`out/scratch/f66f/w`: 7 shapes, main without `<br>` == fixed with and without), i.e. the fix makes a
trailing `<br>` equal to no trailing `<br>`, and what is left is the no-`<br>` behaviour: (a) `un_n49` / `un49_nobr`: a plain
49-line block (1029 px) on a page of 1009 px is not split, `w0048` is at 800-813 pt over the content bottom (799.4 pt), Chrome
splits 47 / 2; the empty fiftieth line used to push the block over whatever tolerance admits a less-than-a-line overflow;
(b) `exact_n46_pb22`, `pd_n47`, `pdt_n47`, `exact_n45_pb22`: every line fits, only the bottom padding and border do not;
Chrome keeps all lines on page 1 (46 / 1, 47 / 1) and ends the box at the top of page 2, we move two lines back (widows) or
put the last line alone on page 2. Neither is in the scope of F66f; filed as F30f-l.

### Not verified

- No Chrome measurement of `<br>` before an `<img>` (the data-URI size problem, F201) or a trailing `<br>` in a multi-column
  box; inline-block / `<svg>` after `<br>` were measured (the line stays; 4 pt strut difference is F183b).
- Parley `main` (0.12) was not checked; the fix does not depend on Parley's behaviour beyond the line count it returns.
- A text whose last cluster is a newline that the breaker did not treat as `BreakReason::Explicit` (for instance a `\r` that is
  the first half of a `\r\n` split across two text nodes) was not constructed.
- fulgur-vrt goldens were not re-blessed (the host does not reproduce 35 of 64).
