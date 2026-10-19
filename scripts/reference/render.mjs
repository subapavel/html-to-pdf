// Renders every fixtures/invoices/*/index.html with headless Chromium into
//   fixtures/invoices/<name>/reference/chrome.pdf      (print-to-PDF, A4, CSS @page respected)
//   fixtures/invoices/<name>/reference/chrome-p<N>.png (each PDF page rasterised at 96 dpi)
// Chrome is the *reference*, not the truth: where fulgur and Chrome disagree, CSS spec decides.
//
// A fixture may carry a `reference.html` next to `index.html`. Chrome renders that one instead, for a document
// whose feature Chrome does not implement (17: `position: running()` / `element()`, which Chrome ignores): the
// reference is the same document written the way Chrome can print it, and the fixture's README says how.
//
// Usage: node scripts/reference/render.mjs [fixture-name ...]
import { chromium } from "playwright";
import { readdir, mkdir, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { execFileSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const invoicesDir = resolve(root, "fixtures/invoices");
const only = new Set(process.argv.slice(2));

const names = (await readdir(invoicesDir, { withFileTypes: true }))
  .filter((d) => d.isDirectory() && existsSync(resolve(invoicesDir, d.name, "index.html")))
  .map((d) => d.name)
  .filter((n) => only.size === 0 || only.has(n))
  .sort();

const browser = await chromium.launch();
try {
  for (const name of names) {
    const dir = resolve(invoicesDir, name);
    const out = resolve(dir, "reference");
    await mkdir(out, { recursive: true });

    const page = await browser.newPage();
    await page.emulateMedia({ media: "print" });
    const source = existsSync(resolve(dir, "reference.html")) ? "reference.html" : "index.html";
    await page.goto(pathToFileURL(resolve(dir, source)).href, { waitUntil: "networkidle" });
    await page.evaluate(() => document.fonts.ready);

    // preferCSSPageSize: honour @page { size; margin } exactly like our engine does.
    const pdf = await page.pdf({ preferCSSPageSize: true, printBackground: true });
    const pdfPath = resolve(out, "chrome.pdf");
    await writeFile(pdfPath, pdf);
    await page.close();

    // Rasterise with pdftoppm (poppler) so the PNGs come from the PDF, not from the screen render.
    execFileSync("pdftoppm", ["-r", "96", "-png", pdfPath, resolve(out, "chrome-p")]);
    const pngs = (await readdir(out)).filter((f) => f.startsWith("chrome-p") && f.endsWith(".png"));
    console.log(`${name}${source === "index.html" ? "" : " (" + source + ")"}: ${(pdf.length / 1024).toFixed(0)} KB, ${pngs.length} page(s)`);
  }
} finally {
  await browser.close();
}
