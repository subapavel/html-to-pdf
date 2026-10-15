# Upstream tracking

What we depend on, what we have reported or patched, and what blocks invoices.

| Layer | Crate / repo | Pinned | Notes |
|---|---|---|---|
| Glue + pagination + PDF | [fulgur](https://github.com/fulgur-rs/fulgur) | 0.40.0 | ZeroVer; breaking per minor |
| DOM, style, layout | [Blitz](https://github.com/DioxusLabs/blitz) (`blitz-dom`) | via fulgur | |
| CSS engine | Stylo (Servo/Firefox) | via Blitz | |
| Box layout | Taffy | via Blitz | no table algorithm — tables approximated over block/flex/grid |
| Text | Parley + HarfBuzz port | via Blitz | |
| PDF writer | Krilla | via fulgur | |

## Known gaps that hit invoices

| Gap | Where | Status | Fixture |
|---|---|---|---|
| `border-collapse: collapse` ignored, shared borders painted twice | Blitz — [#386](https://github.com/DioxusLabs/blitz/issues/386) | open | 03 |
| Inline-level boxes get no background/border/shadow | fulgur draw path | open (documented in fulgur `docs/css-support.md`) | 03 (badges) |
| Nested tables do not fragment | fulgur / Taffy | open | — |
| Determinism depends on host fonts for SVG `<text>` | Blitz `fontdb` global | open | 03 (QR has no text, so unaffected) |

## Our patches / PRs

_(none yet)_
