# ADR-0011: Stress-probe findings go to a backlog; progress is counted over high and medium findings

Date: 2026-11-03 · Status: accepted · Owner decision on the pilot bar: `docs/roadmap.md` (*Pilot bar*)

## Context

Every rendering PR gets an adversarial `vrt-validator` pass over thousands of synthetic shapes (a box one pixel
taller than the page, every paragraph shifted 8 px, `@page { margin: 0 }`). Each pass files three to eight edge cases.
From 2026-10-16 to 2026-11-03 the findings file grew from 58 to 359 entries while 101 were closed. The open high
findings went from 10 (15 on 2026-10-20) to 0, but the open total grew from 54 to 258, about 75 % of them low or
low-medium. A "done / total" percentage therefore falls as validators get more thorough, and says nothing about
whether an invoice prints correctly. The owner asked whether closing every finding is realistic; it is not, and it is
not the goal.

## Decision

1. **Where new findings go.** A new finding whose only repro is a synthetic stress shape *and* whose severity is low or
   low-medium goes to `docs/backlog/stress-findings.md`, not to the findings queue. A finding seen on a fixture, a
   corpus document or an invoice-like layout goes to the queue as before, whatever its severity. Existing entries are
   not moved (that would conflict with every open branch).
2. **What we report.** Progress is reported as closed / all over findings of severity critical, high, medium-high
   and medium in the queue file (`scripts/findings_progress.py`), plus the number of open entries on the roadmap's
   pilot list. The raw total is not reported as progress.
3. **Order.** The pilot list in `docs/roadmap.md` goes first; inside it, and for everything else, the queue line's
   order holds.

## Consequences

- The queue shows the work that can affect a real document; the backlog keeps the edge cases for later.
- A validator may misjudge a shape as synthetic; the coordinator moves an entry to the queue when an invoice hits it.
- The percentage is comparable over time because the script, not a hand count, produces it.
