#!/usr/bin/env python3
"""Progress over the findings queue (ADR-0011).

Counts the `### F…` entries of docs/plans/2026-10-15-m0-findings.md by severity and done state. An entry is done when
its heading says done / fixed or its body has a **Done** / **Fixed** paragraph. Run from the repository root.
"""
import re
import sys
from collections import Counter
from pathlib import Path

FINDINGS = Path("docs/plans/2026-10-15-m0-findings.md")
COUNTED = ("critical", "high", "medium-high", "medium")


def entries(text):
    return re.split(r"\n(?=### F)", text)[1:]


def is_done(entry):
    heading, _, body = entry.partition("\n")
    return bool(
        re.search(r"\b(done|fixed|resolved)\b", heading, re.I)
        or re.search(r"^\s*(\*\*)?(Done|Fixed)\b", body, re.M)
        or re.search(r"\*\*(Done|Fixed)", body)
    )


def severity(entry):
    m = re.search(r"severity: ([a-z-]+)", entry.partition("\n")[0].lower())
    return m.group(1) if m else "?"


def main():
    text = FINDINGS.read_text(encoding="utf-8")
    all_entries = entries(text)
    total, done = Counter(), Counter()
    for e in all_entries:
        s = severity(e)
        total[s] += 1
        if is_done(e):
            done[s] += 1
    counted = sum(total[s] for s in COUNTED)
    closed = sum(done[s] for s in COUNTED)
    for s in sorted(total, key=lambda k: -total[k]):
        print(f"{s:12} {done[s]:4} / {total[s]:4} done")
    print(f"{'all':12} {sum(done.values()):4} / {len(all_entries):4} done")
    pct = 100 * closed / counted if counted else 0.0
    print(f"progress (critical..medium): {closed} / {counted} = {pct:.0f} %")
    return 0


if __name__ == "__main__":
    sys.exit(main())
