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
