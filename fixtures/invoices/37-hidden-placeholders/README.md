# 37 — `visibility: hidden` placeholders, spacers, a hidden link and a hidden draft box (no Tailwind)

One-page Czech invoice written as plain HTML + CSS with an inline `<style>`: the compiled `tailwind.css` has no `.invisible` rule and
rebuilding it would move every fixture. It hides parts of itself the way templates do (CSS 2.1 section 11.2: a hidden box keeps its
space and paints nothing; a `visibility: visible` descendant of it is shown):

- **Sign placeholder:** `30 000,00<span class="sign plus">−</span>` in the right-aligned "Celkem" cells. The `−` is hidden on the rows
  that add and visible on the discount, so the digits of all four amounts end at one x (Chrome: the `000,00` words at x 504.7).
- **Spacer pseudo-element:** `.pad::before { content: "00"; visibility: hidden }` on the KS value, which is two characters shorter than the
  VS value: it must start where `00` would end (Chrome: `0308` at x 339.0), and `00` must not be printed.
- **Hidden inline-block:** a `STORNO` stamp before the title keeps its place in the title line (Chrome: "Faktura" at x 109.2) and is not printed.
- **Hidden block with a visible descendant:** the draft box (`border: 2px dashed`, `background: #fef2f2`, `KONCEPT — neplatný doklad`) is
  hidden; its `<span>` with the signature note is `visibility: visible` and is printed where it stands in the box (Chrome: "Doklad" at
  x 53.2, y 315.1). Neither the box's border and background nor `KONCEPT` appear.
- **Hidden link:** `<a class="screen-only" href=…>Zobrazit online</a>` is hidden: no text and no link annotation (as Chrome).

**On main's binary:** only the box itself was hidden. `Zobrazit online`, the spacer (`000308`), and a `−` on
every row were printed, the signature note was missing (a hidden paragraph with a visible descendant painted nothing) and the link had an
annotation. Compare 3.2 % before the fix.

The test is `hidden_placeholders_invoice_prints_only_what_is_visible` in `crates/htmltopdf/tests/fixtures.rs`; the bare shapes are in
`visibility_hidden.rs`. `−` (U+2212) and `—` are in the bundled Inter. Plan: `docs/plans/2026-11-02-f209-visibility-hidden.md`.
