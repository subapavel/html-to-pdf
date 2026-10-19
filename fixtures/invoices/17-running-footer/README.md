# 17 — a running footer: company details and IBAN on every page (`position: running()` + `@bottom-center`)

Czech invoice of 42 item rows over two pages (the header, party grid and items table of 02), "Celkem k úhradě", and a footer that
is written once, at the top of `<body>`, and printed at the foot of **every** page:

```html
<style>
  .paticka { position: running(paticka); }
  @page { size: A4; margin: 15mm; @bottom-center { content: element(paticka); } }
</style>
…
<footer class="paticka flex justify-between gap-4 border-t border-gray-300 pt-2 text-xs text-gray-500">
  <span>Příkladná integrace s.r.o. · IČO 12345678 · DIČ CZ12345678</span>
  <span>IBAN CZ65 0800 0000 1920 0014 5399 · <a href="https://www.example.cz" …>www.example.cz</a></span>
</footer>
```

This is the GCPM way of putting company details or a logo on every page, and the rule is in an inline `<style>` on purpose: the
Tailwind build cannot declare it (`parse_gcpm` ignores rules inside `@layer`, F105), so the inline path is the one users will hit.

Exercises:
- a running element whose rule reached the document through an inline `<style>` (F102; CSS GCPM 3 §2.2: a running element is not
  displayed where it stands, only where `element()` places it). **On main's binary the footer was also laid out in the body: its
  `border-t` rule was painted above "Faktura" and the whole invoice sat 34 px lower than Chrome's; with a text-only footer its text
  was painted there too, with a second link annotation.** Compare numbers: main's binary 12.7 / 10.0 %; F102 12.0 / 10.0 %.
- a flex container with a link in a margin box, repeated on pages where its source is not (the `first` policy carries it on).

**Reference.** Chrome does not implement `position: running()` or `element()`: it would print the footer in the flow and leave the
margin box empty. `reference.html` is therefore the same invoice with the footer deleted from the body and its two halves printed
as `@bottom-left` / `@bottom-right` strings; `scripts/reference/render.mjs` renders `reference.html` instead of `index.html` when
a fixture has one. What the comparison shows: where the body text sits, the page count (2), that the footer is on both pages in the
bottom margin, and that nothing stands above the first heading. It cannot show the flex layout (Chrome's halves are plain strings,
no top rule, no underlined link).

The remaining diff (12.0 / 10.0 %) is the text drift every fixture has and the `border-b` Chrome paints on every item row (`<tr>`
borders, Blitz #386 / F22: ours rows come out about 0.5 px shorter, so page 1 holds 24 rows against Chrome's 23). The threshold in
`scripts/compare.py` is the worst page + 1 point and goes down with F22.

Every class is in the compiled `fixtures/tailwind/tailwind.css` (no inline styles, no rebuild). The tests that pin the shape without
Tailwind and compare against a document with the footer deleted are in `crates/htmltopdf/tests/running_elements.rs`.
