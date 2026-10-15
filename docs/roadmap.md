# Roadmap

| Milestone | Goal | Done when |
|---|---|---|
| **M0 — oracle** | Five invoice fixtures with Chrome reference renders, pixel-diff script | `python3 scripts/compare.py` prints a number per fixture ✔ |
| **M1 — first light** | `htmltopdf render` produces PDFs for all fixtures; fonts embedded; 01 within a few % of Chrome | CI green, 01 diff under threshold |
| **M2 — pagination** | 02 matches Chrome page breaks, repeated `thead`, "Strana X / Y" | 02 diff under threshold on all 3 pages |
| **M3 — tables & inline** | `border-collapse`, inline badges (upstream PRs or carried patches) | 03 diff under threshold |
| **M4 — colour & effects** | `oklch` conversion verified, gradients, shadows, `print:` variants | 04, 05 within agreed tolerance |
| **M5 — product** | Java (Panama) + Node bindings, HTTP service template+JSON→PDF, PDF/A-3 with ISDOC attachment, QR Platba helper | first real invoice sent to a real customer |
| **M6 — autonomy** | Cloud routines: nightly triage, one-fix-per-night, read-only review | a week of nightly PRs with no human babysitting |

Thresholds are set per fixture in `scripts/compare.py` once M1 lands and we see real numbers.
