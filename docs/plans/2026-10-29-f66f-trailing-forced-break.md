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

## 7.1 Validator round 1: findings and fixes

`vrt-validator` found five defects in the first F66f build (Blitz `6c2da16d`, htmltopdf `be468ab`). Repros: `out/scratch/f66fv/{a..j,k}`,
harness `harness.py DIR`; Chrome is m141. Two of them were caused by F66f, the others by a pre-existing reader that F66f exposed.

| # | Finding | Root cause | Fix |
|---|---|---|---|
| HIGH 1 | Text lost: `<div>a<br style="page-break-after:always"></div>b` gave a blank page and no "a" (also `d2`, `d6`, `d9`, `d11`, `e6`, `e11-13`, `c1`, `c2`, `c7`, `b/pb_inside`); `c6` (`<div>a<span style="break-after:page"></span></div>b`) lost "a" already on main | fulgur's break reader (`column_css::walk`, which fills the side table every `break_before` / `break_after` lookup reads) kept `break-*` on **inline-level** boxes. The pagination walk then entered the inline root, found a zero-height `<br>` / `<span>` child with a forced break, and cut the line at an offset at or past the end of the block | css-break-3 §3.1 (and css-page-3 §5.1, legacy `page-break-*` in CSS 2.1 §13.3.1): the properties apply to block-level boxes, flex and grid items and table parts only. The reader drops `break-before` / `break-after` for an element whose computed outer display is `inline` (`blitz_adapter::is_inline_level_box`). A flex or grid item, a float and a `display:block` span are blockified and keep theirs. Chrome ignores them on inline boxes (one page in `d1`, `c6`, `c7`) |
| HIGH 2 | `e2` (`<p>a<br clear=all style='page-break-before:always'></p><p>b</p>`) drew a and b at the same y; `d3`, `d4` printed b before a; `h7` (`<p><br style="break-after:page"></p><p>b</p>`) two pages against one | same cause | same fix |
| LOW | Word idiom `<p><span><br clear=all style='page-break-before:always'></span></p>` (`g1`, `h3`, `h4`): the next block at 58.6, Chrome 73.8 | same cause: the break cut the paragraph's only line | same fix: 73.6 now |
| MEDIUM | An empty inline with padding or border after `<br>` (`a/br_empty_span_pad`, `br_empty_span_border`) and `<wbr>` (`a/wbr`): AFTER 73.6 on main, 58.6 with F66f, Chrome 73.8 | F66f's takeback checked only for an atomic inline box after the newline | Blitz `3dadc1b1`: `InlineOp::LineContent` marks the start of an inline element with non-zero **inline-axis** margin, padding or border (a percentage or `calc()` counts; `auto` and a border with style `none` do not) and a `<wbr>`; `TextLayout::content_after_last_break` says whether one starts after the last forced break; `break_lines` then keeps the line |

What Chrome does, measured (`out/scratch/f66fv/k`, AFTER below `<div>a<br>X</div>`): `padding:0 4px` 73.8, `border-left:1px` 73.8, `margin:0 4px` 73.8, `padding:1%` 73.8,
`<wbr>` and two `<wbr>` 73.8, with whitespace inside the padded span 73.8; **no line** (58.8) for `padding:4px 0`, `padding-top`, `margin:4px 0`,
`margin-left:0`, an empty `<a id>`, a `display:none` / `hidden` span, and for the closing edge of a padded span that ends in the break
(`<span style="padding:0 4px">x<br></span>`: 73.8 = two lines). So the rule is the inline axis only (Chrome, as css-inline-3 reads
"inline-size > 0"), and the marker is at the start tag, not the end tag. CSS 2.1 §9.4.2 says "margins, padding or borders" without an axis; where
spec and Chrome might differ, `margin-top` / `padding-top` alone (zero-height line in Chrome) was left as Chrome has it.

### What was built

- fulgur fork `subapavel/fulgur` `htmltopdf/srgb-colours` `aba83f75`: `column_css::walk` clears `break_before` / `break_after` of an inline-level
  element before it enters the table; lib test `extract_column_style_table_drops_breaks_on_inline_level_boxes` (`<br>`, `<span>`, inline-block,
  `<img>`, empty `<b>` lose it; `div`, `display:block` span, float, flex item, `tr` keep it; red before), `render_smoke`
  `render_smoke_forced_break_on_an_inline_level_box_is_ignored` (five shapes, one page each, red before). One existing test changed:
  `subtree_facts::anonymous_wrappers_and_mixed_inline_and_block_children` used `<i style="break-before:page">` as the forced break below an
  anonymous wrapper; the break is now in a block inside an inline-block (`<i style="display:inline-block"><div style="break-before:page">`),
  which still gives the wrapper a forced break below.
- Blitz fork `3dadc1b1`: `whitespace.rs` (`InlineOp::LineContent`, `line_content_after_last_break`, a unit test), `construct.rs`
  (`has_inline_axis_pbm` through `stylo_taffy::convert`, set on the generic inline branch before `PushSpan`), `node/element.rs` (the field),
  `inline.rs` (`break_lines` takes the flag; two tests, 14 shapes).
- html-to-pdf: `tests/trailing_break.rs` gained five tests (`a_forced_break_on_an_inline_box_is_ignored_and_loses_no_text`,
  `..._between_paragraphs_does_not_overlap_or_reorder`, `the_word_idiom_...`, `a_forced_break_on_a_block_level_box_still_breaks`,
  `an_inline_box_with_padding_border_or_a_wbr_...`): every printed word with page and top against Chrome (within 1 pt), so a lost word fails.
  Red before (four of the five; the block-level one is the guard), green after. `Cargo.lock`: the two Blitz lines and the fulgur line.
  An inline-block with `break-after` is checked by page and order only: it sits 2 pt below Chrome (F183b strut, not this change).

### Gates

| gate | result |
|---|---|
| Blitz `cargo test -p blitz-dom --lib` (run from the fulgur workspace with the fork patched in; Blitz has no target dir on this host) | 147 passed (144 + 3 new); one earlier run showed the two `log`-capturing `svg_log_tests` failing, a rerun was green (the known flake of §7) |
| fulgur `cargo test -p fulgur --lib` | 2420 passed, 1 failed (`subtree_facts::anonymous_wrappers...`, see above), fixed, that test green afterwards; the other 2420 not rerun after that one-line test edit; `render_smoke` binary: new test green, red without the fix |
| fulgur integration binaries | **not run after the change**: the disk filled up while linking them (see `docs/postmortems/2026-10-29-f66f-disk-full-integration-tests.md`) |
| fulgur-vrt (release, forks patched in) | 35 of 64 differ on this host, as on the base; the diff directory (70 files) is identical to the one of the F66f build |
| `cargo fmt --check` (all three repos), `clippy -D warnings`: fulgur `--all-targets`, blitz-dom `--all-targets` | clean; html-to-pdf clippy and the full `cargo test --locked` **not rerun** (disk) |
| `cargo test --release --locked -p htmltopdf --test trailing_break` | 15 passed |
| byte gate, fixtures 01-33 and the 7 corpus documents (40 files), `f183-gate.sh`, 1 and 4 threads | identical to the F66f build (`be468ab`) at both; 1 and 4 threads identical to each other; against `main` (`htmltopdf-main-f30fh`) only fixture 33 differs, as before |
| `render-fixtures.sh` + `compare.py` | all 33 ok; every number equal to §7's, fixture 33 3.8 % (threshold 4.8 %), no threshold moved |

### Validator batches a-i, j and k against Chrome

`out/scratch/f66fv/{a,b,c,d,e,g,h,i}` (validator's, 192 cases), `j` (30 new: `<input>`, hidden input, `<button>`, `<select>`, `<textarea>`,
inline `<svg>`, `<img>`, `<a id>`, empty `<a href>`, margin-only span, `::before` inline-block after the `<br>`, `font-size:0`,
`visibility:hidden`, `hidden`, `display:none`, abspos and float with padding, zero-width space, nested padding, `display:contents`,
`<wbr>` in a span, ...) and `k` (15: the inline-axis rule above and block-level guards). Per case the distance to Chrome is
(words differing, words on another page, sum of top errors beyond 1 pt); the build is compared with main's binary.

| batch | cases | closer than main | equal | further |
|---|---|---|---|---|
| a-i, j, k | 249 | 170 | 78 | 1 |

Every repro of the validator's report is at Chrome's positions now (error 0.0 over all words): `d1 d2 d3 d4 d6 d9 d11`, `e2 e6 e11 e12 e13`,
`c1 c2 c5 c6 c7`, `b/pb_inside`, `h3 h4 h7`, `g1`, `a/br_empty_span_pad`, `br_empty_span_border`, `wbr`. In `j`, nothing is further than main;
still off, the same as on main: an empty inline-block after the `<br>` and a `::before` inline-block (66.1 / 58.6 against 73.8: the
F183b family), `font-size:0` text (73.6 against 77.6), `<input>` / `<textarea>` / `<button>` (a few pt, F183b / form controls).

The one case that is further, `b/bigpre_nl2` (a `pre` of 150 lines ending in a blank line): AFTER at the top of page 4 (43.6), Chrome 73.8, main 73.6. It was already
so in the F66f build. It is the F30f-l family of §7: the same bytes as main with one trailing `\n` less (checked), so the fix made a trailing
newline equal to none, and what is left is the pagination of a block whose last line fits the page by less than a line (Chrome keeps the
last two lines together on page 4 with the blank one). Filed under F30f-l, not part of this change.

### Not verified

- fulgur's 43 integration binaries and html-to-pdf's full `cargo test --locked` / clippy after this change (disk full, postmortem).
- Nothing was measured for `break-*` on an inline box inside a multi-column container or a table cell, nor for `break-inside`
  on an inline box (the reader keeps it; only before / after were in the report).
- `margin-top` / `padding-top` on an inline element after the `<br>` follows Chrome (no line); CSS 2.1 §9.4.2 read literally keeps the line.
- The side-by-sides `out/scratch/f66fv/sbs_d1.png`, `sbs_e2.png` (main | now | Chrome) were looked at; the fixtures did not move, so there is no
  new golden.

## 7.2 Validator round 2: the end edge of an open ancestor

`vrt-validator` round 2 (repros `out/scratch/f66fv2/p/`) found one MEDIUM regression in the round-1 build (Blitz `3dadc1b1`, htmltopdf `76fd399`):
`<div><span style="padding:0 4px">a<br><span></span></span></div><p>AFTER</p>` (`p/e4`) gave AFTER 58.6; main 73.6, Chrome 73.8. Same for `b_empty`,
`margin`, `nested_deeper`, `ws_inside`, `pre_e4`, `a_btn` (`<a style="padding:4px 8px">a<br><span class="i"></span></a>`). Right already (58.6 = Chrome 58.8):
`plain_outer`, `border` (only `border-left`), `pad_tb`, `closing_pad_only`, `txt_ws`. The round-1 rule looked only at boxes that *start* after the break.

### What Chrome does (m141, `out/scratch/f66fv3/q`, `q2`, `q3`, generator `gen_q.py`; AFTER below `<div>…</div>`, 58.8 = one line, 73.8 = two)

The validator's hypothesis held. The line after the last break is kept when (a) a box starts after the break and (b) the **inline-end** margin, padding
or border of some box that ends on that line is non-zero. The close tags that directly follow the `<br>` (nothing started in between) belong to the line the
break ends. Measured with `A` = the open span and `E` = an empty `<span></span>` after the break:

| shape | AFTER |
|---|---|
| `A{padding-right}` / `margin-right` / `border-right` / `margin-right:-4px` / `padding-right:1%`, `a<br>E` | 73.8 (kept) |
| `A{padding-left}` / `margin-left` / `border-left` / `padding-top` / nothing, `a<br>E` | 58.8 |
| `A{padding-right}` `a<br>` alone; `A{pr}a<br></A>E` (E after A closed); `<span><A{pr}>a<br></A></span>` | 58.8 |
| `<S><A{pr}>a<br></A>E</S>` (outer plain: nothing with an edge ends after E started) | 58.8 |
| `<A{pr}><b>a<br></b>E</A>`, `<A{pr}><b>a<br>E</b></A>`, `<A{pr}>a<br> E </A>`, `<a href style=pr>a<br>E</a>`, `E` = `<i>`, `<a id>` | 73.8 |
| `A{pr}` with `a<br><br>E` | 88.8 (3 lines) |
| `white-space: pre` / `pre-line`, `a\n` + E | 73.8 |
| after the break instead of E: `display:none`, `display:contents`, `<input type=hidden>`, a comment | 58.8 |
| `direction: rtl` (CSS) on the span or its parent: `padding-left` is the end side (kept), `padding-right` is the start side (58.8) | as stated |
| E = float or abspos span (with `A{pr}`) | 73.8 (not handled, below) |

So the line is the box's own **logical** end side, taken from the direction of that box, and the end edge counts only once some box has started on the line.

### Fix (Blitz `67e6be6f`, tests first in `3c0f6ca4`)

`InlineOp::LineContent` is replaced by `InlineOp::BoxStart { edge }` (pushed for every inline element that generates a box, before its `PushSpan`; `edge` is the
inline-start side, or a `<wbr>`) and `InlineOp::EndEdge` (before the `PopSpan` of an element with a non-zero inline-end side). `construct.rs`
`inline_axis_edges(node) -> (start, end)` reads the six lengths as in round 1 (percentage and `calc()` count, `auto` and a `none` border do not) and swaps the
sides for `direction: rtl`. `whitespace.rs::line_content_after_last_break` walks the collapsed ops after the last forced break: `BoxStart { edge: true }` is content,
any `BoxStart` sets "a box started", `EndEdge` is content when a box started. `display:none`, `display:contents` and hidden inputs push no `BoxStart`. Parley is not told
anything. Tests: Blitz `trailing_forced_break_tests` (three new, seven edge styles x eight contexts, one red before; the other two are guards), `whitespace::tests` (two),
`crates/htmltopdf/tests/trailing_break.rs::an_open_box_with_an_end_edge_keeps_the_line_when_a_box_starts_after_the_br` (red before at `padding-right`).

### Evidence

- The 19 repros of `p/`: all at Chrome's AFTER (73.6 / 58.6 against 73.8 / 58.8), the 7 that were wrong in round 1 included.
- `q` (59 shapes), `q3` (6 RTL): every shape of the table above matches Chrome except float / abspos / inline-block after the break (below).
- The validator's batches `n` (167), `p` (19), `r1/a-k` (249), 435 documents against Chrome, `out/scratch/f66fv3/batch.py`: closer to Chrome than main 234, equal 198, further 3.
  Compared with the round-1 build 427 are the same and **none is further** (the 8 that differ moved to Chrome). The three that are further than main: `b/bigpre_nl2`
  (F30f-l, known), `b/long200_p` and `k/k1`, both identical to the round-1 build and artefacts of the sum (page-mismatch words 889 -> 259, now compared on the right
  page; and an inline-block 4 pt F183b strut instead of a missing word).

### Left open (rare, measured, not part of this change)

- An **empty float or abspos span after the `<br>` inside a padded open span** (`P_float`, `P_abs`, `B_float`, `B_abs`): Chrome keeps the line (73.8), we do not (58.6;
  main gave 73.6 for the float by accident and 58.6 for abspos). Blitz treats an inline that contains a block-level box as a block (`is_or_contains_block`), so such a span is
  not in the inline run at all; this is the block-in-inline gap, not the takeback.
- An empty inline-block after the `<br>` (58.6 against 73.8, F183b), unchanged from main.
- The HTML `dir="rtl"` attribute: Blitz does not read it (no presentational hint), so only CSS `direction` flips the end side. Pre-existing; it would show first in the text direction.

### Gates

| gate | result |
|---|---|
| Blitz `cargo test -p blitz-dom --lib` (from the fulgur workspace, forks patched in by `--config`) | 151 passed (147 + 4 new); one run showed `svg_log_tests::a_rejected_inline_svg_is_a_warning` failing on captured debug log lines (the known `log` flake of §7), reruns at 1 and many threads green |
| Blitz `clippy --all-targets -D warnings`, `cargo fmt --check` | clean |
| fulgur `cargo test -p fulgur --lib` | 2421 passed |
| fulgur integration binaries (43), one at a time, executable deleted after each (disk) | all ok |
| html-to-pdf `cargo test --release --locked` (77 test binaries) | 886 passed, 0 failed |
| html-to-pdf `cargo fmt --check`, `cargo clippy --release --locked --workspace --all-targets -- -D warnings` | clean |
| `Cargo.lock` | the two Blitz lines only (`3dadc1b1` -> `67e6be6f`) |
| byte gate, fixtures 01-33 + 7 corpus (40 files), 1 and 4 threads, `out/scratch/f66fv2/gate.sh` | 1 and 4 threads identical; against main only fixture 33 differs (as in round 1); identical to the round-1 build, so fixtures 01-32 and the corpus are byte-identical to main |
| `render-fixtures.sh` + `compare.py` | all 33 ok; fixture 33 3.8 % (threshold 5 % as printed, 4.8 % in the file), every other number equal to main's; side-by-side looked at |
| fulgur-vrt | not rerun (35 of 64 differ on this host as on the base, §7; the change is in Blitz text layout only and moves no fixture or corpus byte) |

### Not verified

- `parallel-construct` feature of Blitz not rerun for this change; the new op is read in the serial part of `build_inline_layout_into`.
- `dir="rtl"` attribute (Blitz ignores it), vertical writing modes, float / abspos after the break (above).
- Chrome only at A4 / Inter 14 px / 20 px line height; no measurement in a multi-column box or a table cell for the new shapes (the takeback is the same function there).
