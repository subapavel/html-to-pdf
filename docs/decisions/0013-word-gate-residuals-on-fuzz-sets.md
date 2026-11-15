# ADR-0013: The per-token word gate is zero on fixtures and curated sets; on random fuzz sets a documented residual may merge

Date: 2026-11-15 · Status: accepted (owner's decision, 2026-11-15) · First use: F213b (PR #103,
`docs/plans/2026-11-14-f213b-split-box-sharing-a-line.md` §7.15)

## Context

A pagination PR is gated on words: a word that main prints must not be lost, counted per token (not net), synthetic documents
included. Since F213b the validator also runs random generators (`gen`, `gen2`, `gen3`, hundreds to thousands of documents per
seed) whose documents are far from invoice layouts: boxes wider than a column inside multicol, negative margins on boxes
taller than a page, content thousands of points past the sheet. On such sets a large pagination change moves content between
pages in hundreds of documents, and in a handful a token that main happened to place on a sheet ends up off it, while the same
document prints far more of its words than main. F213b took four rounds to go from 174 such documents to 11 of 7316; in round 4
every narrow fallback for the last ones lost words in other documents (plan §7.14).

## Decision

The owner chose this on 2026-11-15 when asked about F213b.

1. **Zero, no exceptions:** fixtures, corpus documents and the curated sets (`rvw/sh`, `rvw/sw`, `rv/var`, `rv/var2`, the
   F213d / F213e families, and any set a plan names). A loss there is fixed on the branch.
2. **Random fuzz sets:** a PR may merge with residual documents when **all** of these hold:
   - the residual is under 0.5 % of the fuzz documents run, including at least one seed that no fix was tuned on;
   - every residual document was read and is listed in the plan with its tokens and where main and the PR put them;
   - each one is a token pushed past the sheet edge by content that overflows the sheet or its column, a line traded at the
     page edge where any placement loses a line, or a checker artefact; and the PR prints more of that document's words than
     main overall;
   - none is a word missing from the content stream without a warning, and no word is printed twice;
   - the residual is filed as one backlog entry (ADR-0011) that names the classes and the documents.
3. A residual that fails any of these is fixed on the branch, however many rounds it takes.

## Consequences

The gate stays absolute on documents that look like what users send. A large engine change is not held back by generator
output it improves overall, and the residual stays visible in the backlog for the next agent.
