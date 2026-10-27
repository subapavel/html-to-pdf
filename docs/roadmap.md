# Roadmap

| Milestone | Goal | Done when |
|---|---|---|
| **M0 — oracle** | Five invoice fixtures with Chrome reference renders, pixel-diff script | `python3 scripts/compare.py` prints a number per fixture ✔ |
| **M1 — first light** | `htmltopdf render` produces PDFs for all fixtures; fonts embedded; 01 within a few % of Chrome | CI green, 01 diff under threshold |
| **M2 — pagination** | 02 matches Chrome page breaks, repeated `thead`, "Strana X / Y" | 02 diff under threshold on all 3 pages |
| **M3 — tables & inline** | `border-collapse`, inline badges (carried patches in our forks) | 03 diff under threshold |
| **M4 — colour & effects** | `oklch` conversion verified, gradients, shadows, `print:` variants | 04, 05 within agreed tolerance |
| **M5 — product** | Java (Panama) + Node bindings, HTTP service template+JSON→PDF, PDF/A-3 with ISDOC attachment, QR Platba helper | first real invoice sent to a real customer |
| **M6 — autonomy** | Cloud routines: nightly triage, one-fix-per-night, read-only review | a week of nightly PRs with no human babysitting |

*Owner decision 2026-10-27: a 200-page table of items (`bench/corpus/b200-items.html`) at about 1.2 s on the dev VM is
accepted; the 1 s goal stays for other 200-page documents. Table speed work is no longer a priority (F99).*

*Owner decision 2026-10-26: PDF/A-3 and e-invoicing (ISDOC / ZUGFeRD attachments) are not needed for now; do not plan them until the owner asks.*

Thresholds are set per fixture in `scripts/compare.py` once M1 lands and we see real numbers.
