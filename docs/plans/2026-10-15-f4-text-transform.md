# F4 — Apply `text-transform`

Date: 2026-10-15. Finding: F4 in `2026-10-15-m0-findings.md`.

## Problem

Blitz 0.2.4 pushes text nodes into Parley's tree builder as written (`layout/construct.rs`, `NodeData::Text`), so
`text-transform` has no effect. Tailwind's `uppercase` (table headers, card labels, 05's "FAKTURA · DAŇOVÝ DOKLAD")
printed in source case, both on the page and in the extracted text.

## Spec

CSS Text 3 §2.1: `uppercase` / `lowercase` use the Unicode full case mappings (`ß` → `SS`); `capitalize` puts the
first typographic letter unit of each word in title case; the transform happens before whitespace processing and
line breaking, so it affects layout and the text a reader copies. Language-sensitive mappings (Turkish `i`) are
recommended, not required.

## Change (Blitz fork, `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`)

In `build_inline_layout_recursive`, a text node takes `text-transform` from its parent element's computed style and
is pushed transformed. `capitalize` keeps its at-word-start state in a thread-local reset per inline layout, so word
boundaries carry across inline elements (`wo<b>rd</b>` stays one word); letters and digits continue a word, an
apostrophe only inside one (`don't`, `'Citát'`); combining marks and soft hyphens stay inside a word; `<br>` starts
a new word.

Not done (found by `vrt-validator` on PR #9, all low): `::before`/`::after` generated text is not transformed (it
does not go through the text-node path); language-specific mappings (`lang="tr"`: `i` → `İ`); Greek final sigma
across text nodes; `full-width` / `full-size-kana`; SVG `<text>`.

All blitz-dom fixes now live on one branch, `htmltopdf/blitz-dom-0.2.4` (F1's commits + this one), because
`[patch.crates-io]` takes one source per crate. The F1-only branch `htmltopdf/svg-current-color-srgb` is superseded.

## Acceptance

- `crates/htmltopdf/tests/text_transform.rs` (pdftotext): uppercase incl. `ß` → `SS` and Czech diacritics,
  inheritance into `<span>`, `none` resetting, lowercase, capitalize across inline elements and punctuation,
  Tailwind's `.uppercase` on a `<th>`. Red before, green after.
- Compare: 01 4.44 → 4.36, 02 10.35 → 10.35, 03 8.93 → 8.92, 04 5.83 → 5.75, 05 21.51 → 21.57 %. The 05 growth
  (+0.06 pp) is the labels and headers now in capitals, as in Chrome, drawn at positions shifted by the page-margin
  difference (F6); checked visually. Under merge-policy item 3 (content now rendered as the reference shows it);
  05 stays under its threshold (0.22).
