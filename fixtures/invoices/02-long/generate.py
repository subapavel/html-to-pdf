#!/usr/bin/env python3
"""Generates index.html for fixture 02-long (deterministic, 60 line items).
Run from anywhere: python3 fixtures/invoices/02-long/generate.py
"""
from pathlib import Path
import random

random.seed(2026)
services = [
    "Analýza rozhraní {n} – mapování IDoc → XML",
    "Vývoj iFlow {n} – SOAP adaptér, WS-Security",
    "Oprava mapování {n} – chybějící pole v segmentu E1EDK01",
    "Konzultace architektury {n} – groovy script vs. XSLT",
    "Testování rozhraní {n} – end-to-end scénář s partnerem",
    "Monitoring a podpora {n} – řešení incidentu P2",
    "Migrace ICO {n} z PO 7.5 do Integration Suite",
    "Dokumentace rozhraní {n} – runbook pro provoz",
]
def czk(x):  # 12 345,67
    return f"{x:,.2f}".replace(",", "\u00a0").replace(".", ",")

rows = []
subtotal = 0.0
for i in range(1, 61):
    desc = random.choice(services).format(n=f"IF-{1000+i}")
    qty = random.choice([1, 2, 3, 4, 6, 8, 12])
    unit = random.choice([1800.0, 2200.0, 2500.0, 2900.0])
    total = qty * unit
    subtotal += total
    rows.append(f"""    <tr class="border-b border-gray-100 break-inside-avoid">
      <td class="py-1.5 pr-2 text-gray-500 tabular-nums">{i}</td>
      <td class="py-1.5 pr-2">{desc}</td>
      <td class="py-1.5 px-2 text-right tabular-nums">{qty} h</td>
      <td class="py-1.5 px-2 text-right tabular-nums">{czk(unit)}</td>
      <td class="py-1.5 pl-2 text-right tabular-nums">{czk(total)}</td>
    </tr>""")

vat = subtotal * 0.21
html = f"""<!doctype html>
<html lang="cs">
<head>
<meta charset="utf-8">
<title>Faktura 2026-0002</title>
<link rel="stylesheet" href="../../tailwind/tailwind.css">
<style>
  @page {{
    size: A4;
    margin: 15mm 15mm 20mm 15mm;
    @bottom-right {{
      content: "Strana " counter(page) " / " counter(pages);
      font-family: Inter, sans-serif; font-size: 9pt; color: #6b7280;
    }}
    @bottom-left {{
      content: "Faktura 2026-0002 · Příkladná integrace s.r.o.";
      font-family: Inter, sans-serif; font-size: 9pt; color: #6b7280;
    }}
  }}
  /* Tailwind has no break-* utilities for print; plain CSS. */
  thead {{ display: table-header-group; }}
  tr {{ break-inside: avoid; }}
  .keep-together {{ break-inside: avoid; }}
</style>
</head>
<body class="font-sans text-[10pt] text-gray-900 leading-snug">

<header class="flex items-start justify-between border-b-2 border-gray-900 pb-3 mb-6">
  <div>
    <h1 class="text-2xl font-bold tracking-tight">Faktura</h1>
    <p class="text-sm text-gray-600">Daňový doklad č. <span class="font-semibold text-gray-900">2026-0002</span> · období 09/2026</p>
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

<table class="w-full text-sm">
  <thead>
    <tr class="border-b border-gray-300 text-left text-xs uppercase tracking-wider text-gray-500">
      <th class="py-2 pr-2 font-semibold w-8">#</th>
      <th class="py-2 pr-2 font-semibold">Popis</th>
      <th class="py-2 px-2 font-semibold text-right w-16">Množ.</th>
      <th class="py-2 px-2 font-semibold text-right w-24">Cena/h</th>
      <th class="py-2 pl-2 font-semibold text-right w-28">Celkem</th>
    </tr>
  </thead>
  <tbody>
{chr(10).join(rows)}
  </tbody>
</table>

<section class="keep-together flex justify-end mt-6">
  <table class="text-sm w-80">
    <tbody>
      <tr><td class="py-1 text-gray-600">Základ daně</td><td class="py-1 text-right tabular-nums">{czk(subtotal)} Kč</td></tr>
      <tr><td class="py-1 text-gray-600">DPH 21 %</td><td class="py-1 text-right tabular-nums">{czk(vat)} Kč</td></tr>
      <tr class="border-t-2 border-gray-900 text-lg font-bold"><td class="pt-2">Celkem k úhradě</td><td class="pt-2 text-right tabular-nums">{czk(subtotal+vat)} Kč</td></tr>
    </tbody>
  </table>
</section>

<section class="keep-together mt-8 text-sm text-gray-600">
  <p>Bankovní účet <span class="font-mono text-gray-900">123456789/0300</span>, variabilní symbol <span class="font-mono text-gray-900">20260002</span>. Děkujeme za spolupráci.</p>
</section>

</body>
</html>
"""
out = Path(__file__).with_name("index.html")
out.write_text(html.replace(",", " ").replace("  ", ", ") if False else html, encoding="utf-8")
print(f"wrote {out} ({len(rows)} rows, subtotal {subtotal:,.2f})")
