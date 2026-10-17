# 11 — full-width rows

One-page Czech invoice with the "colspan wide enough for any table" template idiom: the items table has four columns
(`Popis` auto, `Množ.` `w-20`, `Cena` `w-28`, `Celkem` `w-32`), and its section headers (`Služby`, `Licence a materiál`),
the VAT summary, the total and the notes are single cells with `colspan="99"`.

Exercises:
- a `colspan` far wider than the table does not create columns: in automatic layout a column in which no cell starts has
  no width, and a spanning cell covers the columns that exist (Chrome; HTML calls such a column a table model error and
  defines no rendering). Before F59 the 99 spanned columns were `auto` grid tracks that took a share of the width, and the
  description column collapsed to a few words per line while the header row stopped at 405 px
- the full-width rows are as wide as the table and the real columns stand where Chrome puts them
- one page, like Chrome

Only classes present in `fixtures/tailwind/tailwind.css` are used (`w-20`, `w-28`, `w-32`, `border-collapse`, `bg-gray-50`,
`bg-gray-100`, `bg-gray-900`, `text-white`, `tabular-nums`, `leading-relaxed`, ...). `border-collapse` itself is not
implemented in Blitz (#386), so cell borders are painted per cell.
