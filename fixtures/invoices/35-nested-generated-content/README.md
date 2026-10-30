# 35 — generated content in inline elements nested in other inline elements, on one page

One-page Czech invoice (fixture 33's header, parties and items table) whose generated content sits in an inline element that is
itself inside another inline element, the way a template engine nests its spans. All rules are in an inline `<style>` (as fixture
22), so `tailwind.css` is not rebuilt.

- item amounts and the total: `<td><span class="price"><span class="amount">45 000,00</span></span></td>`, `.amount::after { content: " Kč" }`;
- a status badge: `Stav: <span class="badge"><span class="dot"></span>Uhrazeno</span>`, `.dot::before` a 6 px inline-block green dot;
- `Datum zdanitelného plnění: <strong><span class="req">15. 10. 2026</span></strong>`, `.req::after { content: "*" }`, and the legend
  `* údaj podle § 29 zákona o DPH`;
- a note in Czech quotes in a plain `<span><span class="q">…</span></span>`, `::before { content: "„" }` / `::after { content: "“" }`
  (string literals, not `<q>`);
- `<a href><span class="ext">platba.example.cz</span></a>`, `.ext::after { content: " →" }`.

Exercises:
- CSS 2.1 §12.1: `::before` / `::after` are boxes just inside their host, wherever the host sits in the tree. Chrome: "Kč" at 540.9 pt
  (536.4 for the total), the dot pushes "Uhrazeno" to 78.9 pt, `2026*` at 214.0 pt, the quotes around the note, the arrow after the link.
- Every generated glyph (`Kč`, `„ “`, `→`, `*`) is in the bundled Inter; `--strict-fonts` fails otherwise.

**On main's binary:** every one of these was missing (no text, no dot) and "Uhrazeno" stood 8.5 pt too far left; the amounts were
right-aligned without their currency. The pseudo-element of a host directly in a block was printed, one level deeper it was not.

The tests are in `crates/htmltopdf/tests/nested_pseudos.rs` and `fixtures.rs`
(`nested_generated_content_invoice_prints_every_generated_string`). Plan: `docs/plans/2026-10-30-f66i-nested-inline-pseudos.md`.
