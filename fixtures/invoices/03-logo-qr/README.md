# 03 — logo, QR Platba, badges, collapsed borders

Exercises the features known or suspected to be weak in fulgur/Blitz:
- `<img>` PNG logo with `h-12 w-auto` (intrinsic aspect ratio)
- inline `<span>` badges with `bg-*`, `rounded-full`, `px-3` — inline-level backgrounds
- `border-collapse` table with a border on every cell — fulgur docs say unsupported (`DioxusLabs/blitz#386`): expect doubled borders
- `<tfoot>` with `colspan="4"`
- inline `<svg>` QR code (QR Platba / SPD 1.0 format) sized by the parent (`w-full h-full`)
- `grid-cols-[auto_1fr]` arbitrary grid template for `<dl>`
- `rounded-lg border` card

Assets are generated, not downloaded: `logo.png` is a 316-byte synthetic PNG, QR is produced by `segno`.
