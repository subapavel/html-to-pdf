# 51 — a box of 80 numbered terms (`inline-block`, taller than a page) on the third line of the "Obchodní podmínky" paragraph, between `Souhlasím:` and `Podpis odběratele`

Three-page Czech invoice. Header (grid), four item rows, the total, then ONE `<p class="obchodni">` of three lines: two lines of
text (`Obchodní podmínky tvoří přílohu … pod posledním bodem.`), a `<br>`, and the third line `Souhlasím: [box] Podpis odběratele ________`
where the box is `<span class="podminky inline-block">` with 80 block children `1. Faktura je splatná …` … `80. Spory řeší soud …`
(340 px wide, 1 px grey border, 8 pt text on 18 px lines: 1440 px, 1.4 pages). `.obchodni` and `.podminky` are in the fixture's `<style>`
(no Tailwind rebuild). A closing line `Děkujeme za spolupráci.` and a footer follow.

Exercises (html-to-pdf F213b):
- css-break-3 §4.1: a line box too large for its fragmentainer may be split and the atomic inlines on it fragmented. The line that holds
  the box is taller than a page and is not the first of its paragraph, so it starts a page of its own (Chrome), and the box is cut
  between its terms.
- CSS 2.1 §10.8.1: the baseline of an `inline-block` is the baseline of its last line box. `Souhlasím:` and `Podpis odběratele` sit on the
  baseline of term 80, on the box's last page, and the closing line follows below.
- Chrome m153 (the committed reference): page 1 the invoice and the two intro lines (`Obchodní` at 328.1 pt); page 2 the box from its
  top frame, term 1 at 47.8 pt, the frame open at the bottom; page 3 the rest of the terms, `Souhlasím:` and `Podpis` at 354.3 pt level
  with `80.` (356.8 pt), `Děkujeme` at 373.8 pt. Chrome prints the term at each page edge twice (a sliced box); we print each once.

**On the binary before the change (main, fork 4be3de79):** the box is one oversized leaf on page 2 (its terms 56 to 80 and the line's
words `Souhlasím:` and `Podpis` are below the page edge: 318 pt of content past it, the renderer warns), pixel diff 1.9 / 9.3 / 3.3 %.
After: terms 1 to 55 on page 2, 56 to 80 on page 3 with `Souhlasím:` and `Podpis` level with `80.`, 1.8 / 4.5 / 4.9 %; threshold 5.9 %.
What is not Chrome's: our page 2 holds term 55 as its last (Chrome 56, which it prints again on page 3), and the box's frame is closed
at the bottom of page 2 and at the top of page 3 (Chrome slices it open).

The test is `terms_box_beside_label_is_cut_and_its_line_is_on_the_last_page` in `crates/htmltopdf/tests/fixtures.rs`. Plan:
`docs/plans/2026-11-14-f213b-split-box-sharing-a-line.md`.
