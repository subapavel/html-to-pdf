#!/usr/bin/env python3
"""Benchmark and speed guard for the htmltopdf release CLI.

    python3 scripts/bench.py [document ...]            print the table
    python3 scripts/bench.py --check                   ... and fail on a regression (CI)
    python3 scripts/bench.py --update-baseline         rewrite bench/baseline.json from this run
    python3 scripts/bench.py --generate                rewrite the corpus under bench/corpus/ (deterministic)
    python3 scripts/bench.py --scaling                 ms at 1200 / 2400 / 4800 rows for the table variants, and r = t(4800)/t(1200)

Options: --runs N (timed runs per gated document, default 7; one untimed warm-up first),
         --slow-runs N (runs per report-only document, default 1, no warm-up), --skip-slow.
Env:     HTMLTOPDF_BIN  binary to measure (default target/release/htmltopdf), as in render-fixtures.sh.

Documents (plan docs/plans/2026-10-18-b1-benchmark.md):
  fixtures 01-16      fixtures/invoices/*, rendered with the same flags as scripts/render-fixtures.sh
  b200-*              ~200-page documents in bench/corpus/ (items table, terms text, mixed)   -- gated
  slow-*              known slow shapes (F82, F30d-d, F83f, F87f)                              -- report only

Measure: wall clock of one `htmltopdf render` process per run (that is how customers call it), min / median / max.
Gate (--check), on gated documents only (fixtures and b200-*):
  * hard budgets, any machine: a fixture's median <= HARD_MS_FIXTURE, a b200-* median <= HARD_MS_B200
    (HARD_MS_OVERRIDE for a document already over it on main, with the reason);
  * relative, only when the machine label equals the one in bench/baseline.json:
        median <= baseline * REL + ABS_MS.
A document over its limit is measured once more with twice the runs and the lower median counts.
Scaling gate (F99c): one long table must cost linear time per row. Variant `t` (the b200-items table without margin boxes) is
rendered with 1200 and 4800 rows; r = t(4800) / t(1200) is about 4 when the work is linear and tends to 16 when it is quadratic.
`--check` fails when r > SCALING_MAX_RATIO (re-measured once with twice the runs, the lower r counts); it is self-normalising,
so it holds on any machine. `--scaling` prints all variants and sizes and does not gate by itself.
Raising a baseline follows the compare.py rule: only for content that was missing and now renders, with the cause named.
The corpus is committed so the numbers do not move when this generator changes; --generate must be byte-reproducible.
"""
from __future__ import annotations

import argparse
import json
import os
import random
import re
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCH = ROOT / "bench"
CORPUS = BENCH / "corpus"
BASELINE = BENCH / "baseline.json"
OUT = ROOT / "out" / "bench"
FIXTURES = ROOT / "fixtures" / "invoices"

REL = 1.30  # allowed growth over the baseline median ...
ABS_MS = 5.0  # ... plus this much (process-start noise on 10 ms documents)
HARD_MS_FIXTURE = 100.0  # AGENTS.md: single-page invoice in low tens of ms
HARD_MS_B200 = 1500.0  # AGENTS.md goal is 1000 ms on the dev VM; this is the any-machine limit (plan §2) ...
HARD_MS_OVERRIDE = {
    # ... except where main is already over it. One 4800-row table with margin boxes (counter(pages) in the footer) takes
    # about 1.6 s for 200 pages on the dev VM since F99b (2.3 s before; docs/benchmarks.md): the goal is not met for this
    # shape, so the limit (about 1.8x the median, a CI runner is slower) is a backstop against a collapse, not the goal.
    # Lower it in the PR that makes the shape faster.
    "b200-items": 3000.0,
}
GOAL_MS_B200 = 1000.0  # reported, not gated
# t(4800 rows) / t(1200 rows) of variant `t`: linear is about 4, quadratic tends to 16 (F99c). Main measured 4.5-5.0, F99c's
# branch 4.0-4.4 (median of 5-10 runs, 4 vCPU; the plan's "halfway" 4.4 failed on a re-measure of 4.40), so 4.6 is the
# first value that does not flake on the branch. It catches a return of the per-page table scans only partly (they add
# about 0.4-0.6 to r); a worse regression (r towards 5 and more) fails.
SCALING_MAX_RATIO = 4.6
SCALING_ROWS = (1200, 2400, 4800)
# The scaling variants: the b200-items table (gen_items arguments). `t` is gated; the others say where a superlinear cost is.
SCALING_VARIANTS = {
    "t": {"boxes": False},  # the corpus table, plain @page
    "t-sep": {"boxes": False, "collapse": False},  # border-collapse: separate
    "t-noavoid": {"boxes": False, "avoid": False},  # no row-level break-inside: avoid
    "t-nohead": {"boxes": False, "thead": False},  # no repeated header
    "t-boxes": {},  # the corpus shape: with the two margin boxes
}
SCALING_GATED = "t"
B200_PAGES = (195, 205)  # a b200-* document that leaves this range no longer measures "200 pages"
TIMEOUT_S = 180

# Sizes tuned with pdfinfo so that every b200-* comes out at about 200 pages (checked in the table).
N_ITEM_ROWS = 4800
N_TEXT_SECTIONS = 1230
N_MIXED_UNITS = 181
SLOW_PARAGRAPH_WORDS = 40000  # F82: 20 000 words took 2.5 s in F78's measurement; 120 000 would take minutes
SLOW_MULTICOL_WORDS = 20000  # F30d-d: the same shape in `columns:2`
SLOW_BREAK_RULES, SLOW_BREAK_ELEMENTS = (24000, 1000)  # F87f: rules x elements
SLOW_AVOID_LEVELS, SLOW_AVOID_CHILDREN = (400, 20)  # F83f: nested break-inside:avoid levels x children per level

# --------------------------------------------------------------------------------------------------------------------
# Corpus generator. Fixed seeds, no clock, no dict-order dependence: two runs give byte-identical files.
# Czech text is from the fixtures (16-terms-card clauses, 02-long item descriptions).
# --------------------------------------------------------------------------------------------------------------------

CLAUSES = [
    "Tyto obchodní podmínky se vztahují na dodávky programového vybavení, na jeho údržbu a na související konzultační činnost, které dodavatel poskytuje odběrateli na základě objednávky potvrzené touto fakturou.",
    "Předmětem plnění je dílo nebo služba popsaná v objednávce; každá změna rozsahu vyžaduje písemné potvrzení obou stran a může změnit cenu i termín dodání podle ceníku platného v den potvrzení změny.",
    "Dodavatel předá dílo odběrateli v elektronické podobě na dohodnuté úložiště; za okamžik předání se považuje chvíle, kdy odběratel obdrží e-mailem odkaz ke stažení a přístupové údaje potřebné k jeho otevření.",
    "Odběratel provede přejímku do deseti pracovních dnů od předání; vady, které nebrání řádnému užívání díla, nejsou důvodem k odmítnutí převzetí a dodavatel je odstraní v rámci záruky podle těchto podmínek.",
    "Nevyjádří-li se odběratel ve lhůtě pro přejímku, považuje se dílo za převzaté bez výhrad dnem, kdy tato lhůta marně uplynula, a dodavatel je oprávněn vystavit konečné vyúčtování.",
    "Licence k programovému vybavení se uděluje jako nevýhradní, nepřenosná a časově neomezená pro počet pracovišť uvedený v objednávce; rozšíření počtu pracovišť se sjednává dodatkem a účtuje se podle ceníku.",
    "Odběratel není oprávněn programové vybavení dále pronajímat, upravovat ani zpětně analyzovat, pokud právní předpis nestanoví jinak; zdrojové kódy zůstávají majetkem dodavatele a nejsou předmětem dodání.",
    "Záruční doba činí dvanáct měsíců od převzetí díla; po dobu záruky dodavatel bezplatně odstraňuje vady, které odběratel uplatní písemně a které nevznikly nesprávným používáním nebo zásahem třetí osoby.",
    "Údržba zahrnuje opravy chyb, drobné úpravy v reakci na změny právních předpisů a technickou podporu v pracovní dny od osmi do šestnácti hodin; novými funkcemi se údržba nezabývá, ty se objednávají zvlášť.",
    "Reakční doba na hlášení kritické chyby, která znemožňuje provoz, je čtyři pracovní hodiny; u ostatních chyb dodavatel zahájí řešení nejpozději následující pracovní den po jejich nahlášení.",
    "Chyby se hlásí prostřednictvím podpůrného portálu nebo e-mailem na adresu uvedenou v záhlaví faktury; hlášení musí obsahovat popis chyby, kroky k jejímu zopakování a údaj o použité verzi.",
    "Odběratel zajistí zálohování svých dat a odpovídá za to, že před každým zásahem dodavatele do jeho systémů existuje aktuální záloha, ze které lze původní stav obnovit bez zbytečného odkladu.",
    "Dodavatel nenese odpovědnost za ztrátu dat, která vznikla tím, že odběratel zálohu nepořídil nebo ji nezkontroloval, ani za škodu způsobenou nesprávným zadáním vstupních údajů obsluhou odběratele.",
    "Cena díla se skládá z ceny za vývoj, ceny za licenci a ceny za nasazení; ceny jsou uvedeny bez daně z přidané hodnoty, kterou dodavatel připočte v sazbě platné ke dni uskutečnění zdanitelného plnění.",
    "Dílo nad rozsah pěti pracovních dnů se fakturuje po etapách podle harmonogramu; každá etapa se považuje za samostatné plnění, jehož cena je splatná bez ohledu na dokončení ostatních etap.",
    "Zálohová faktura je splatná do sedmi dnů od vystavení; do zaplacení zálohy dodavatel není povinen zahájit práce, a termíny dodání se v takovém případě prodlužují o dobu prodlení se zaplacením.",
    "Pokud odběratel zruší objednávku po zahájení prací, uhradí dodavateli cenu již provedených činností a prokazatelně vynaložené náklady na zajištění zdrojů, které již nelze využít jinak.",
    "Dodavatel může při plnění použít komponenty s otevřenou licencí; jejich seznam a licenční podmínky předá odběrateli spolu s dílem a odběratel se zavazuje tyto podmínky dodržovat při užívání díla.",
    "Důvěrnými informacemi jsou veškeré technické a obchodní údaje, které si strany předají v souvislosti s plněním; strany je nesmějí zpřístupnit třetí osobě, ani využít k jinému účelu než k plnění této smlouvy.",
    "Povinnost mlčenlivosti se nevztahuje na informace, které jsou veřejně známé bez porušení této povinnosti, které strana získala nezávisle na druhé straně nebo které je povinna sdělit na základě zákona či rozhodnutí orgánu.",
    "Zpracovává-li dodavatel osobní údaje jménem odběratele, uzavřou strany smlouvu o zpracování; bez ní dodavatel k osobním údajům odběratele nepřistupuje a v testovacích prostředích používá pouze anonymizovaná data.",
    "Kontaktními osobami jsou osoby uvedené v objednávce; změnu kontaktní osoby oznámí strana druhé straně e-mailem a změna je účinná doručením oznámení, nestanoví-li oznámení pozdější den.",
    "Dodavatel může pověřit plněním podukodavatele, za jehož práci odpovídá, jako by ji vykonal sám; odběratel s tím souhlasí, pokud podukodavatel přijal závazek mlčenlivosti ve stejném rozsahu jako dodavatel.",
    "Strana, které bylo plnění znemožněno okolností vylučující odpovědnost, je povinna to druhé straně neprodleně oznámit; po dobu trvání takové okolnosti se lhůty pro plnění přiměřeně prodlužují.",
]
SERVICES = [
    "Analýza rozhraní {n} – mapování IDoc → XML",
    "Vývoj iFlow {n} – SOAP adaptér, WS-Security",
    "Oprava mapování {n} – chybějící pole v segmentu E1EDK01",
    "Konzultace architektury {n} – groovy script vs. XSLT",
    "Testování rozhraní {n} – end-to-end scénář s partnerem",
    "Monitoring a podpora {n} – řešení incidentu P2",
    "Migrace ICO {n} z PO 7.5 do Integration Suite",
    "Dokumentace rozhraní {n} – runbook pro provoz",
]
TITLES = [
    "Předmět plnění", "Odborná péče", "Součinnost odběratele", "Cena a platební podmínky", "Předání a přejímka",
    "Záruka a údržba", "Odpovědnost za škodu", "Důvěrnost informací", "Ochrana osobních údajů", "Závěrečná ustanovení",
]

PAGE_PLAIN = "@page { size: A4; margin: 15mm; }"
PAGE_BOXES = """@page {
    size: A4;
    margin: 15mm 15mm 20mm 15mm;
    @bottom-right {
      content: "Strana " counter(page) " / " counter(pages);
      font-family: Inter, sans-serif; font-size: 9pt; color: #6b7280;
    }
    @bottom-left {
      content: "Faktura {no} · Příkladná integrace s.r.o.";
      font-family: Inter, sans-serif; font-size: 9pt; color: #6b7280;
    }
  }"""
HEADER = """<header class="flex items-start justify-between border-b-2 border-gray-900 pb-3 mb-6">
  <div>
    <h1 class="text-2xl font-bold tracking-tight">Faktura</h1>
    <p class="text-sm text-gray-600">Daňový doklad č. <span class="font-semibold text-gray-900">{no}</span></p>
  </div>
  <div class="text-right text-sm">
    <p><span class="text-gray-500">Vystaveno:</span> 1. 10. 2026</p>
    <p><span class="text-gray-500">Splatnost:</span> 31. 10. 2026</p>
  </div>
</header>

<section class="grid grid-cols-2 gap-8 mb-6 text-sm">
  <div>
    <h2 class="text-xs font-semibold uppercase tracking-wider text-gray-500 mb-1">Dodavatel</h2>
    <p class="font-semibold">Příkladná integrace s.r.o.</p><p>Krátká 12, 541 01 Trutnov</p><p>IČO 12345678 · DIČ CZ12345678</p>
  </div>
  <div>
    <h2 class="text-xs font-semibold uppercase tracking-wider text-gray-500 mb-1">Odběratel</h2>
    <p class="font-semibold">Zákazník a syn, a.s.</p><p>Dlouhá třída 1234/5, 110 00 Praha 1</p><p>IČO 87654321 · DIČ CZ87654321</p>
  </div>
</section>
"""
THEAD5 = """  <thead>
    <tr class="border-b border-gray-300 text-left text-xs uppercase tracking-wider text-gray-500">
      <th class="py-2 pr-2 font-semibold w-8">#</th>
      <th class="py-2 pr-2 font-semibold">Popis</th>
      <th class="py-2 px-2 font-semibold text-right w-16">Množ.</th>
      <th class="py-2 px-2 font-semibold text-right w-24">Cena/h</th>
      <th class="py-2 pl-2 font-semibold text-right w-28">Celkem</th>
    </tr>
  </thead>"""


def czk(x: float) -> str:
    return f"{x:,.2f}".replace(",", " ").replace(".", ",")


def page(title: str, css: str, body: str) -> str:
    return (
        '<!doctype html>\n<html lang="cs">\n<head>\n<meta charset="utf-8">\n'
        f"<title>{title}</title>\n<style>\n  {css}\n</style>\n</head>\n"
        '<body class="font-sans text-[10pt] text-gray-900 leading-snug">\n\n'
        f"{body}\n</body>\n</html>\n"
    )


def item_rows(rng: random.Random, n: int, first: int = 1, avoid: bool = True) -> tuple[list[str], float]:
    rows, subtotal = [], 0.0
    cls = "border-b border-gray-100 break-inside-avoid" if avoid else "border-b border-gray-100"
    for i in range(first, first + n):
        desc = rng.choice(SERVICES).format(n=f"IF-{1000 + i}")
        qty = rng.choice([1, 2, 3, 4, 6, 8, 12])
        unit = rng.choice([1800.0, 2200.0, 2500.0, 2900.0])
        subtotal += qty * unit
        rows.append(
            f'    <tr class="{cls}">'
            f'<td class="py-1.5 pr-2 text-gray-500 tabular-nums">{i}</td>'
            f'<td class="py-1.5 pr-2">{desc}</td>'
            f'<td class="py-1.5 px-2 text-right tabular-nums">{qty} h</td>'
            f'<td class="py-1.5 px-2 text-right tabular-nums">{czk(unit)}</td>'
            f'<td class="py-1.5 pl-2 text-right tabular-nums">{czk(qty * unit)}</td></tr>'
        )
    return rows, subtotal


def gen_items(rows: int = N_ITEM_ROWS, boxes: bool = True, collapse: bool = True, avoid: bool = True,
              thead: bool = True) -> str:
    """The 02-long shape x N: one items table with a repeated thead, row-level break-inside:avoid, margin boxes.
    The defaults are the committed b200-items document; the other values are the scaling variants (F99c, --scaling):
    `boxes` the two margin boxes (else a plain @page), `collapse` Tailwind's border-collapse (else `separate`),
    `avoid` the row-level break-inside:avoid, `thead` the header row."""
    rng = random.Random(20260001)
    rows, subtotal = item_rows(rng, rows, avoid=avoid)
    vat = subtotal * 0.21
    body = HEADER.format(no="2026-B001") + f"""
<table class="w-full text-sm">
{THEAD5 + chr(10) if thead else ""}  <tbody>
{chr(10).join(rows)}
  </tbody>
</table>

<section class="keep-together flex justify-end mt-6">
  <table class="text-sm w-80">
    <tbody>
      <tr><td class="py-1 text-gray-600">Základ daně</td><td class="py-1 text-right tabular-nums">{czk(subtotal)} Kč</td></tr>
      <tr><td class="py-1 text-gray-600">DPH 21 %</td><td class="py-1 text-right tabular-nums">{czk(vat)} Kč</td></tr>
      <tr class="border-t-2 border-gray-900 text-lg font-bold"><td class="pt-2">Celkem k úhradě</td><td class="pt-2 text-right tabular-nums">{czk(subtotal + vat)} Kč</td></tr>
    </tbody>
  </table>
</section>

<section class="keep-together mt-8 text-sm text-gray-600">
  <p>Bankovní účet <span class="font-mono text-gray-900">123456789/0300</span>, variabilní symbol <span class="font-mono text-gray-900">2026B001</span>. Děkujeme za spolupráci.</p>
</section>
"""
    css = (
        (PAGE_BOXES.replace("{no}", "2026-B001") if boxes else PAGE_PLAIN)
        + "\n  thead { display: table-header-group; }"
        + ("\n  tr { break-inside: avoid; }" if avoid else "")
        + "\n  .keep-together { break-inside: avoid; }"
        + ("" if collapse else "\n  table { border-collapse: separate; }")
    )
    return page("Faktura 2026-B001", css, body)


def clauses(rng: random.Random, n: int) -> str:
    return " ".join(rng.choice(CLAUSES) for _ in range(n))


def gen_text() -> str:
    """The 06 / 13 shapes x N: titled terms sections kept whole (F83b) and, every 25th, a body-level avoid paragraph
    taller than a page (F83: relaxed, breaks between lines)."""
    rng = random.Random(20260002)
    parts = []
    for i in range(1, N_TEXT_SECTIONS + 1):
        parts.append(
            '<section class="break-inside-avoid mb-4 text-xs">\n'
            f'  <h3 class="font-semibold text-gray-900">{i}. {rng.choice(TITLES)}</h3>\n'
            f'  <p class="text-gray-600">{clauses(rng, rng.choice([2, 3, 4]))}</p>\n'
            "</section>"
        )
        if i % 25 == 0:
            parts.append(f'<p class="text-xs text-gray-600 break-inside-avoid mb-4">{clauses(rng, 37)}</p>')
    body = HEADER.format(no="2026-B002") + "\n".join(parts) + "\n"
    return page("Faktura 2026-B002", PAGE_PLAIN, body)


def gen_mixed() -> str:
    """Per unit: a table (thead + rows), a two-column terms block (F30d), an overflow-hidden card whose paragraph
    crosses the page (F30f), and a relative box with an absolutely positioned stamp and a note (F30g)."""
    rng = random.Random(20260003)
    parts = []
    nxt = 1
    for u in range(1, N_MIXED_UNITS + 1):
        rows, subtotal = item_rows(rng, 8, nxt)
        nxt += 8
        cols = "\n".join(
            f'  <h3 class="font-semibold text-gray-900">{k}. {rng.choice(TITLES)}</h3>\n'
            f'  <p class="text-gray-600">{clauses(rng, 1)}</p>'
            for k in range(1, 7)
        )
        parts.append(
            f"""<section class="mb-6">
<h2 class="text-base font-bold mb-2">Doklad {u}</h2>
<table class="w-full text-sm mb-4">
{THEAD5}
  <tbody>
{chr(10).join(rows)}
  </tbody>
</table>
<p class="text-right text-lg font-bold mb-4">Celkem <span class="tabular-nums ml-1">{czk(subtotal)} Kč</span></p>
<section class="gap-8 text-xs mb-4" style="columns:2">
{cols}
</section>
<section class="overflow-hidden rounded-lg border border-gray-300 mb-4 text-sm">
  <h2 class="bg-gray-50 border-b border-gray-300 px-4 py-2 text-xs font-semibold uppercase tracking-wider text-gray-500">Obchodní podmínky {u}</h2>
  <p class="p-4">{clauses(rng, 6)}</p>
</section>
<div style="position:relative;padding-bottom:2rem">
  <div class="text-lg font-bold uppercase" style="position:absolute;top:0;right:0;border:3px solid #16a34a;color:#15803d;padding:0.25rem 0.75rem">Zaplaceno</div>
  <p class="text-sm text-gray-600">Bankovní účet <span class="font-mono text-gray-900">123456789/0300</span>, variabilní symbol <span class="font-mono text-gray-900">{20260000 + u}</span>.</p>
</div>
</section>"""
        )
    body = HEADER.format(no="2026-B003") + "\n".join(parts) + "\n"
    css = PAGE_PLAIN + "\n  thead { display: table-header-group; }\n  tr { break-inside: avoid; }"
    return page("Faktura 2026-B003", css, body)


def words(rng: random.Random, n: int) -> str:
    pool = " ".join(CLAUSES).split()
    return " ".join(rng.choice(pool) for _ in range(n))


def gen_slow_paragraph() -> str:
    """F82: one very long <p>."""
    rng = random.Random(20260004)
    body = HEADER.format(no="2026-S001") + f'<p class="text-sm text-gray-600">{words(rng, SLOW_PARAGRAPH_WORDS)}</p>\n'
    return page("Faktura 2026-S001", PAGE_PLAIN, body)


def gen_slow_multicol() -> str:
    """F30d-d: one very long <p> inside `columns:2`."""
    rng = random.Random(20260005)
    body = HEADER.format(no="2026-S002") + (
        f'<section class="text-xs text-gray-600" style="columns:2"><p>{words(rng, SLOW_MULTICOL_WORDS)}</p></section>\n'
    )
    return page("Faktura 2026-S002", PAGE_PLAIN, body)


def gen_slow_breaks() -> str:
    """F87f: a sheet of many layered break rules over many elements (the reader tests every rule against every element)."""
    rng = random.Random(20260006)
    props = ["break-inside:avoid", "break-inside:auto", "break-before:auto", "break-after:auto", "break-before:avoid", "break-after:avoid"]
    n_layers = 50
    per = SLOW_BREAK_RULES // n_layers
    layers = []
    for layer in range(n_layers):
        rules = "".join(f".k{layer * per + r}{{{rng.choice(props)}}}" for r in range(per))
        layers.append(f"@layer l{layer}{{{rules}}}")
    elems = []
    for e in range(SLOW_BREAK_ELEMENTS):
        cls = " ".join(f"k{rng.randrange(per * n_layers)}" for _ in range(3))
        elems.append(f'<p class="{cls} text-xs text-gray-600">{rng.choice(CLAUSES)[:90]}</p>')
    body = HEADER.format(no="2026-S003") + "\n".join(elems) + "\n"
    css = PAGE_PLAIN + "\n  " + "\n  ".join(layers)
    return page("Faktura 2026-S003", css, body)


def gen_slow_avoid() -> str:
    """F83f: nested break-inside:avoid levels, each with several children."""
    rng = random.Random(20260007)
    out = ""
    for level in range(SLOW_AVOID_LEVELS, 0, -1):
        kids = "\n".join(f'<p class="text-xs text-gray-600">{rng.choice(CLAUSES)[:40]}</p>' for _ in range(SLOW_AVOID_CHILDREN))
        out = f'<div class="break-inside-avoid">\n{kids}\n{out}</div>\n'
    body = HEADER.format(no="2026-S004") + out
    return page("Faktura 2026-S004", PAGE_PLAIN, body)


GENERATORS = {
    "b200-items": gen_items,
    "b200-text": gen_text,
    "b200-mixed": gen_mixed,
    "slow-paragraph": gen_slow_paragraph,
    "slow-multicol": gen_slow_multicol,
    "slow-breaks": gen_slow_breaks,
    "slow-avoid": gen_slow_avoid,
}


def generate() -> int:
    CORPUS.mkdir(parents=True, exist_ok=True)
    for name, fn in GENERATORS.items():
        data = fn().encode("utf-8")
        (CORPUS / f"{name}.html").write_bytes(data)
        print(f"wrote bench/corpus/{name}.html  {len(data) / 1024:8.1f} kB")
    return 0


def scaling_doc(variant: str, rows: int) -> Doc:
    """Writes out/bench/scaling/<variant>-<rows>.html (not committed) and returns it as a measurable document."""
    d = OUT / "scaling"
    d.mkdir(parents=True, exist_ok=True)
    path = d / f"{variant}-{rows}.html"
    path.write_text(gen_items(rows=rows, **SCALING_VARIANTS[variant]), encoding="utf-8")
    return Doc(f"{variant}-{rows}", "scaling", path)


def scaling_ratio(variant: str, runs: int) -> tuple[dict[int, dict], float | None]:
    """Measures every size of `variant` (1 warm-up, `runs` timed); returns the results and r, or None on an error."""
    res = {n: measure(scaling_doc(variant, n), runs, warmup=True) for n in SCALING_ROWS}
    lo, hi = res[SCALING_ROWS[0]], res[SCALING_ROWS[-1]]
    if lo["error"] or hi["error"]:
        return res, None
    return res, hi["median_ms"] / lo["median_ms"]


def scaling_report(runs: int, check: bool, full: bool) -> list[str]:
    """Prints the scaling table (all variants when `full`, else only the gated one). Returns the --check failures."""
    names = list(SCALING_VARIANTS) if full else [SCALING_GATED]
    head = (f"{'variant':12} " + " ".join(f"{str(n) + ' rows':>11}" for n in SCALING_ROWS)
            + f" {'r=t(' + str(SCALING_ROWS[-1]) + ')/t(' + str(SCALING_ROWS[0]) + ')':>15} {'pages':>6}")
    out, failures = ["", f"scaling: median ms of {runs} runs, r = {SCALING_ROWS[-1]}-row time / {SCALING_ROWS[0]}-row time "
                     f"(linear about {SCALING_ROWS[-1] / SCALING_ROWS[0]:.0f}); gate: variant {SCALING_GATED} r <= {SCALING_MAX_RATIO}", head], []
    print("\n".join(out), flush=True)
    for v in names:
        res, r = scaling_ratio(v, runs)
        note = ""
        if r is None:
            err = next(x["error"] for x in res.values() if x["error"])
            failures.append(f"scaling {v}: render failed: {err}")
            line = f"{v:12} ERROR {err}"
        else:
            if check and v == SCALING_GATED and r > SCALING_MAX_RATIO:  # confirm once with twice the runs; the lower r counts
                res2, r2 = scaling_ratio(v, runs * 2)
                if r2 is not None and r2 < r:
                    res, r = res2, r2
                    note = "  (re-measured)"
            if check and v == SCALING_GATED:
                if r > SCALING_MAX_RATIO:
                    failures.append(f"scaling {v}: r = {r:.2f} > {SCALING_MAX_RATIO} (work per row is not linear in the number of rows)")
                    note += "  FAIL"
                else:
                    note += f"  ok (<= {SCALING_MAX_RATIO})"
            pages = res[SCALING_ROWS[-1]]["pages"]
            line = (f"{v:12} " + " ".join(f"{res[n]['median_ms']:11.1f}" for n in SCALING_ROWS)
                    + f" {r:15.2f} {pages if pages is not None else '?':>6}{note}")
        out.append(line)
        print(line, flush=True)
    (OUT / "scaling.txt").write_text("\n".join(out) + "\n")
    return failures


# --------------------------------------------------------------------------------------------------------------------
# Measurement
# --------------------------------------------------------------------------------------------------------------------

class Doc:
    def __init__(self, name: str, kind: str, path: Path):
        self.name, self.kind, self.path = name, kind, path

    @property
    def gated(self) -> bool:
        return self.kind in ("fixture", "b200")


def documents() -> list[Doc]:
    docs = [Doc(p.name, "fixture", p / "index.html") for p in sorted(FIXTURES.iterdir()) if (p / "index.html").exists()]
    for name in GENERATORS:
        docs.append(Doc(name, "b200" if name.startswith("b200-") else "slow", CORPUS / f"{name}.html"))
    return docs


def binary() -> str:
    return os.environ.get("HTMLTOPDF_BIN") or str(ROOT / "target/release/htmltopdf")


def render_cmd(doc: Doc, pdf: Path) -> list[str]:
    cmd = [binary(), "render", str(doc.path.relative_to(ROOT)), "-o", str(pdf.relative_to(ROOT)),
           "--css", "fixtures/tailwind/tailwind.css"]
    for f in sorted((ROOT / "fixtures/fonts").glob("Inter-*.ttf")):
        cmd += ["--font", str(f.relative_to(ROOT))]
    if doc.kind == "fixture":
        for img in sorted(list(doc.path.parent.glob("*.png")) + list(doc.path.parent.glob("*.jpg"))):
            cmd += ["--image", f"{img.name}={img.relative_to(ROOT)}"]
    cmd += ["--title", doc.name, "--lang", "cs", "--strict-fonts", "--strict-images"]
    return cmd


def timed(cmd: list[str]) -> tuple[float, subprocess.CompletedProcess | None]:
    t0 = time.perf_counter()
    try:
        p = subprocess.run(cmd, cwd=ROOT, capture_output=True, timeout=TIMEOUT_S)
    except subprocess.TimeoutExpired:
        return (time.perf_counter() - t0) * 1000, None
    return (time.perf_counter() - t0) * 1000, p


def pdf_pages(pdf: Path) -> int | None:
    if not shutil.which("pdfinfo"):
        return None
    p = subprocess.run(["pdfinfo", str(pdf)], capture_output=True, text=True)
    m = re.search(r"^Pages:\s+(\d+)", p.stdout, re.M)
    return int(m.group(1)) if m else None


def measure(doc: Doc, runs: int, warmup: bool) -> dict:
    pdf = OUT / f"{doc.name}.pdf"
    cmd = render_cmd(doc, pdf)
    res: dict = {"name": doc.name, "kind": doc.kind, "runs": runs, "samples_ms": [], "error": None}
    if warmup:
        _, p = timed(cmd)
        if p is None or p.returncode != 0:
            res["error"] = "timeout" if p is None else f"exit {p.returncode}: {p.stderr.decode(errors='replace').strip()[-300:]}"
            return res
    for _ in range(runs):
        ms, p = timed(cmd)
        if p is None:
            res["error"] = "timeout"
            return res
        if p.returncode != 0:
            res["error"] = f"exit {p.returncode}: {p.stderr.decode(errors='replace').strip()[-300:]}"
            return res
        res["samples_ms"].append(ms)
    s = res["samples_ms"]
    res.update(min_ms=min(s), median_ms=statistics.median(s), max_ms=max(s), pages=pdf_pages(pdf), kb=pdf.stat().st_size / 1024)
    return res


def startup(runs: int) -> float:
    s = []
    for _ in range(runs + 1):
        ms, _p = timed([binary(), "--version"])
        s.append(ms)
    return statistics.median(s[1:])


def machine_label() -> str:
    model = "unknown cpu"
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                model = line.split(":", 1)[1].strip()
                break
    except OSError:
        pass
    n = len(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else (os.cpu_count() or 0)
    return f"{n}x {model}"


def limits(doc: Doc, base: dict | None, same_machine: bool) -> tuple[float, str]:
    hard = HARD_MS_FIXTURE if doc.kind == "fixture" else HARD_MS_OVERRIDE.get(doc.name, HARD_MS_B200)
    lim, why = hard, f"hard budget {hard:.0f} ms"
    if same_machine and base and doc.name in base.get("documents", {}):
        b = base["documents"][doc.name]["median_ms"]
        rel = b * REL + ABS_MS
        if rel < lim:
            lim, why = rel, f"baseline {b:.1f} ms x {REL:.2f} + {ABS_MS:.0f} ms"
    return lim, why


def fmt_row(r: dict, gate: str) -> str:
    if r["error"]:
        return f"{r['name']:23} {r['kind']:8} ERROR {r['error']}"
    return (f"{r['name']:23} {r['kind']:8} {r['pages'] if r['pages'] is not None else '?':>5} {r['min_ms']:9.1f} {r['median_ms']:10.1f} "
            f"{r['max_ms']:9.1f} {r['kb']:9.1f} {r['runs']:>4}  {gate}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("names", nargs="*", help="documents to run (default: all)")
    ap.add_argument("--check", action="store_true", help="fail on a regression or a broken budget")
    ap.add_argument("--update-baseline", action="store_true", help="write bench/baseline.json from this run")
    ap.add_argument("--generate", action="store_true", help="rewrite the corpus under bench/corpus/ and exit")
    ap.add_argument("--runs", type=int, default=7)
    ap.add_argument("--slow-runs", type=int, default=1)
    ap.add_argument("--skip-slow", action="store_true")
    ap.add_argument("--scaling", action="store_true",
                    help="print the scaling variants (ms at 1200/2400/4800 rows, r); only these, unless documents are named too")
    a = ap.parse_args()

    if a.generate:
        return generate()

    bin_ = binary()
    if a.scaling and not a.names:
        if not (os.path.isfile(bin_) and os.access(bin_, os.X_OK)):
            print(f"no binary at {bin_}: cargo build --release -p htmltopdf-cli (or set HTMLTOPDF_BIN)", file=sys.stderr)
            return 2
        if not (ROOT / "fixtures/fonts/Inter-Regular.ttf").exists():
            subprocess.run([str(ROOT / "scripts/fetch-fonts.sh")], check=True)
        OUT.mkdir(parents=True, exist_ok=True)
        failures = scaling_report(a.runs, a.check, full=True)
        for f in failures:
            print(f"  {f}")
        return 1 if failures else 0
    if not (os.path.isfile(bin_) and os.access(bin_, os.X_OK)):
        print(f"no binary at {bin_}: cargo build --release -p htmltopdf-cli (or set HTMLTOPDF_BIN)", file=sys.stderr)
        return 2
    if not (ROOT / "fixtures/fonts/Inter-Regular.ttf").exists():
        subprocess.run([str(ROOT / "scripts/fetch-fonts.sh")], check=True)
    docs = [d for d in documents() if (not a.names or d.name in a.names) and not (a.skip_slow and d.kind == "slow")]
    missing = [d.name for d in docs if not d.path.exists()]
    if missing:
        print(f"missing corpus files {missing}: python3 scripts/bench.py --generate", file=sys.stderr)
        return 2
    OUT.mkdir(parents=True, exist_ok=True)

    machine = machine_label()
    base = json.loads(BASELINE.read_text()) if BASELINE.exists() else None
    same = bool(base) and base.get("machine") == machine
    if a.check and not base:
        print("note: no bench/baseline.json, only the hard budgets apply")
    elif a.check and not same:
        print(f"note: machine '{machine}' differs from the baseline's '{base['machine']}': only the hard budgets apply")

    results: dict[str, dict] = {}
    failures: list[str] = []
    lines = [f"{'document':23} {'kind':8} {'pages':>5} {'min ms':>9} {'median ms':>10} {'max ms':>9} {'out kB':>9} {'runs':>4}  gate"]
    print(lines[0], flush=True)
    for d in docs:
        gated = d.gated
        r = measure(d, a.runs if gated else a.slow_runs, warmup=gated)
        gate = ""
        if r["error"]:
            if r["error"] != "timeout" or gated:
                failures.append(f"{d.name}: render failed: {r['error']}")
            gate = "ERROR"
        elif gated:
            lim, why = limits(d, base, same)
            if r["median_ms"] > lim:  # confirm once with twice the runs; the lower median counts
                r2 = measure(d, a.runs * 2, warmup=False)
                if not r2["error"] and r2["median_ms"] < r["median_ms"]:
                    r2["retried_from_ms"] = r["median_ms"]
                    r = r2
            if r["median_ms"] > lim:
                gate = "FAIL"
                failures.append(f"{d.name}: median {r['median_ms']:.1f} ms > {lim:.1f} ms ({why})")
            else:
                gate = f"ok (<= {lim:.0f})"
            if d.kind == "b200" and r["pages"] is not None and not (B200_PAGES[0] <= r["pages"] <= B200_PAGES[1]):
                gate += f"  NOTE pages {r['pages']} outside {B200_PAGES[0]}-{B200_PAGES[1]}"
        else:
            gate = "report only"
        results[d.name] = r
        lines.append(fmt_row(r, gate))
        print(lines[-1], flush=True)

    su = startup(a.runs)
    lines.append(f"\nCLI start-up (`htmltopdf --version`, median of {a.runs}): {su:.1f} ms")
    for d in docs:
        r = results[d.name]
        if d.kind == "b200" and not r["error"]:
            lines.append(f"AGENTS.md goal (200 pages under {GOAL_MS_B200:.0f} ms): {d.name} {r['median_ms']:.0f} ms "
                         + ("met" if r["median_ms"] <= GOAL_MS_B200 else "NOT met on this machine"))
    lines.append(f"machine: {machine}; binary: {bin_}")
    print("\n".join(lines[-(len(lines) - len(docs) - 1):]))
    (OUT / "table.txt").write_text("\n".join(lines) + "\n")
    (OUT / "results.json").write_text(json.dumps({"machine": machine, "binary": bin_, "startup_ms": su, "results": results}, indent=2) + "\n")

    if a.update_baseline:
        if a.names or a.skip_slow:
            print("--update-baseline needs a full run (no names, no --skip-slow)", file=sys.stderr)
            return 2
        BASELINE.write_text(json.dumps({
            "machine": machine, "runs": a.runs, "slow_runs": a.slow_runs, "startup_ms": round(su, 1),
            "documents": {n: {"kind": r["kind"], "pages": r["pages"], "min_ms": round(r["min_ms"], 1), "median_ms": round(r["median_ms"], 1)}
                          for n, r in results.items() if not r["error"]},
        }, indent=2) + "\n")
        print(f"wrote {BASELINE.relative_to(ROOT)}")

    if a.check and not a.names:  # the scaling gate; a run of named documents is a quick look and skips it
        failures += scaling_report(a.runs, True, full=a.scaling)
    elif a.scaling:
        failures += scaling_report(a.runs, a.check, full=True)

    if a.check:
        if failures:
            print("\nbench --check FAILED:")
            for f in failures:
                print(f"  {f}")
            return 1
        print("\nbench --check ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
