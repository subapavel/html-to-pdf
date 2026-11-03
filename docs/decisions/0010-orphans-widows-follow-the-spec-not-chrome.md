# ADR-0010: A paragraph with no allowed break between its lines moves whole, as css-break-3 requires; we do not copy Chrome's split

Date: 2026-11-02 · Status: accepted · First use: F30c-o3 (`docs/plans/2026-11-02-f30c-o3-orphans-widows-relaxation.md`); closes F30c-w

## Context

Suppose a paragraph of `n` lines fits a page, but only `room` lines are left on the current page. No break between its
lines leaves at least `orphans` lines before the break and at least `widows` after it (`n < orphans + widows`, or
`widows > n`). Chrome m141 still splits it. Over 2,940 body-level probes (F30c-o `vrt-validator`, A4, 14 px / 21 px
Inter), the number of lines Chrome puts on page 1 always follows this rule:

- `n` if `room >= n`;
- 0 if `room < orphans`;
- otherwise `min(room, max(orphans, n - widows))`.

So with `orphans: 3; widows: 3` a 5-line paragraph with room for 3 lines goes 3 / 2. With `orphans: 1; widows: 5` a
4-line paragraph with room for 2 goes 1 / 3. With the initial 2 / 2, a 3-line paragraph with room for 2 goes 2 / 1
(F30c-w). Since F30c-o our engine moves these paragraphs whole to the next page. Before F30c-o it used 2 / 2 whatever
the author wrote, so for these author values it matched Chrome by coincidence.

AGENTS.md says Chrome is the reference, not the truth, and asks for the spec section before we change anything. The
`css-spec-researcher` pass of 2026-11-02 quoted the following. Re-check the exact wording before quoting it elsewhere.

- **css-break-3 §4.4 Rule 3**: "Breaking at a class B break point is allowed only if the number of line boxes between
  the break and the start of the enclosing block box is the value of orphans or more, and the number of line boxes
  between the break and the end of the box is the value of widows or more."
- **css-break-3 §4.4 relaxation**: "If the above doesn't provide enough break points to keep content from overflowing
  the fragmentainer, then rule 3 is dropped to provide more break points."
- **css-break-3 §4.1**: the break between sibling block boxes (Class A) is a possible break point. The break before the
  paragraph is allowed when its `break-before` / `break-after` values are `auto` (Rule 1).
  - After that break the paragraph fits the next page, so the allowed breaks are enough and Rule 3 is not dropped.
- **css-break-3 §3.3**, in a normative paragraph: "If a block contains fewer lines than the value of widows or orphans,
  the rule simply becomes that all lines in the block must be kept together."
- **CSS 2.1 §13.3.3 Rule C**: says the same as Rule 3. The §13.3.5 example says: "it cannot be split … so it should
  move as a block to the next page".
- **css-break-3 §4.5**: the "equally filled" guideline chooses only among allowed breaks.

Other engines:

- Chrome's own engine test `printing/page-break-orphans-and-widows.html` (inherited from WebKit) asserts the
  move-whole result for these shapes. Its committed baselines in Blink and WebKit record the result as **FAIL**, and
  have done so since at least 2018, so Chrome's split is a known, long-standing bug.
- WeasyPrint moves the paragraph whole (`layout/block.py`, read only, not run).
- Firefox does not implement `orphans` / `widows`.

## Decision

1. Rule 3 is a hard rule as long as the content has another allowed break that keeps it from overflowing. A paragraph
   that fits a page and has no allowed Class B break where it starts moves whole (the Class A break before it). This
   holds for author values and for the initial 2 / 2.
2. Rule 3 is relaxed only where css-break-3 §4.4 allows it: the content cannot be kept from overflowing with the
   allowed breaks. That covers:
   - a paragraph taller than a page;
   - every strip after the first fragment;
   - a paragraph whose preceding break is forbidden by `break-before` / `break-after: avoid` or an ancestor's
     `break-inside: avoid`. css-break-3 drops Rule 3 before Rules 1, 2 and 4; CSS 2.1 drops them in the reverse order,
     and the CSSWG resolved for the css-break-3 order in September 2015.

   When we relax, the order is Chrome's: the author's rules, then keep the orphans and drop the widows, then the
   natural break with the initial 2 orphans and no widows (F30c-o3 validator round 1: Chrome drops Rule 3 altogether when the
   orphans cannot be kept, 49 lines with `orphans: 49` give 48 / 1, not 47 / 2). The spec leaves that choice to the UA, and WPT `css-break/widows-orphans-018.html` pins
   keep-the-orphans in a continuation fragment.
3. Where this makes a fixture differ from its Chrome reference, the threshold comment in `scripts/compare.py` names
   this ADR. Each test that pins one of these shapes names it as well and gives Chrome's numbers. That way the next
   agent does not "fix" it towards Chrome.

## Consequences

- F30c-w is closed as "Chrome non-conformance; the spec wins". Its pinning test
  `pre_advance_split::a_three_line_paragraph_moves_whole_where_chrome_gives_up_the_widows_rule` stays.
- Item 1 of F30c-o3 ("short paragraphs, regression") is not a regression. The 194 grid cases where we differ from
  Chrome are conforming.
- F30c-o3 keeps the cases where the spec does allow relaxing (tall paragraphs, keep-the-orphans in the nested splitter)
  and `orphans` larger than a page.
- Revisit this decision if Chrome changes its behaviour, or if the CSSWG changes Rule 3 or §3.3.

## Clarification (2026-11-02, F215 keep-with-next)

Item 2c ("a paragraph whose preceding break is forbidden by `break-before` / `break-after: avoid` ...") is read together with
its lead-in sentence: Rule 3 is relaxed only where the allowed breaks cannot keep the content from overflowing, **and no
earlier allowed break (for example the break before the chain's head) keeps the content inside the fragmentainer**. A heading
with `break-after: avoid` that has content above it on the page always has an allowed Class A break in front of it, so when
heading and the box after it fit a page the pair moves together and nothing is relaxed: a 5-line paragraph with
`orphans: 3; widows: 3` stays whole (0 / 5 with the heading on page 2), where Chrome keeps the orphans (3 / 2). The relaxation does start
where no earlier break helps: the heading is first on its page, or the chain is taller than a page. There Rule 3 gives way first, in Chrome's
order (Blink ranks a break that violates `break-avoid` below one that violates orphans and widows): 1 / 4, 2 / 3, 3 / 2 and 3 / 2 for room for
1 to 4 lines after a heading that fills the page, and 60 three-line paragraphs with `break-after: avoid` page 47 / 48 / 48 / 37 lines.
`avoid-column` constrains no page break (css-break-3 §3.1). Plan `docs/plans/2026-11-02-f215-keep-with-next.md`; tests
`keep_with_next.rs`, `orphans_widows::a_short_paragraph_after_an_avoid_break_moves_with_its_heading`.
