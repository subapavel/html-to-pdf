# ADR-0003: Re-baseline a fixture's threshold when a fix brings its page count to the reference

Date: 2026-10-15 · Status: accepted · First use: F5 (`docs/plans/2026-10-15-f5-table-column-widths.md`)

## Context

`scripts/compare.py` measures the share of differing pixels per page and gates on the worst page. The standing
decision allows a threshold to rise only when content that was missing is now rendered. It did not foresee a fix
that changes a fixture's page count: page N of the old render and page N of the new render hold different rows, so
their per-page numbers are not comparable. In F5, 02-long went from 4 pages (Chrome: 3) to 3 pages; the sum of the
per-page diffs went down, but the worst page went up (10.4 % → 11.8 %), because rows now sit next to Chrome's rows
and drift against them by Chrome's collapsed row borders and its repeated table header instead of being a whole
page off.

## Decision

When a fix makes a fixture listed in `PAGE_COUNT_MAY_DIFFER` match the reference page count, the same PR removes the
entry and sets that fixture's threshold to the new worst page plus a 1-point margin, with a comment in `compare.py`
naming the cause of the remaining diff and the finding that will bring it down. From then on the normal rule applies
(lower on improvement, never raise except for content that was missing).

This applies only to fixtures in `PAGE_COUNT_MAY_DIFFER`; a page-count change anywhere else is still a failure.

## Consequences

- A correct pagination fix is never blocked by a metric that compares different pages.
- The PR must show the per-page numbers before and after and the side-by-side PNG, so a real regression hidden in
  the re-baseline is visible to the validator.
