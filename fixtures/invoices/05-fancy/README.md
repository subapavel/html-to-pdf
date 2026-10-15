# 05 — fancy ("screen-first" design printed)

What a designer who has never printed anything would ship. Expected to look wrong in fulgur today;
this fixture measures *how* wrong, and which of these matter for invoices (few).

Exercises:
- `@page { margin: 0 }` with full-bleed header and `px-[15mm]` content padding (arbitrary values with units)
- `bg-gradient-to-r` / `bg-gradient-to-br` (linear gradients, `oklch` interpolation)
- `print:bg-slate-900` variant — must apply under the `print` medium, must NOT under `screen`
- `shadow-md`, `shadow-xl shadow-indigo-500/30` (coloured shadows with alpha)
- `ring-1` (box-shadow based borders)
- `rounded-xl` + `overflow-hidden` clipping a table's corners
- `even:bg-gray-50/60` (alpha colour), `hover:` (must be ignored)
- `divide-y`, `space-y-1`, `border-white/30`, `border-dashed`
- `size-5` SVG icon with `currentColor` and `opacity-80`
- `text-balance`, `underline decoration-indigo-300 underline-offset-2`
- `tracking-[0.2em]` arbitrary letter-spacing, `text-4xl font-extrabold`
