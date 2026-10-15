# 04 — bilingual CZ/EN, multi-rate VAT, reverse charge

A typical B2B export invoice.

Exercises:
- `grid-cols-4` with `col-span-2` mixed spans
- two tables side by side in `grid-cols-5` (`col-span-3` + `col-span-2`), `self-start`
- `divide-y` (Tailwind's `:where(& > :not(:last-child))` selector — a good test of Stylo selector support)
- `align-top` in table cells, `<br>` + nested `text-xs` span inside a cell
- `normal-case tracking-normal` overriding an ancestor's `uppercase tracking-wider` inside `<th>`
- Tailwind v4 `oklch()` palette (`indigo-*`) — colour-space conversion to PDF RGB
- `border-l-4` accent, `rounded-md bg-indigo-50` panel with `text-3xl` figure
- German `ß`, `⇄`, `Σ`, `€` glyphs — font coverage / fallback
