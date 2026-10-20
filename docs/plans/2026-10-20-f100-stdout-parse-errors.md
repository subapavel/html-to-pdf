# F100: html5ever parse errors printed on stdout

Date: 2026-10-20. Finding: F100 (`2026-10-15-m0-findings.md`). Status: done (§7).
Forks: `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4` (after `a4b7624`, the commit in `Cargo.lock:193`). The fulgur fork does not change. Line references are to the working trees at `/home/user/blitz` and `/home/user/fulgur`, and to the crates.io sources in `~/.cargo/registry`.

## 1. Problem

`blitz-html` 0.2.0 collects every non-fatal html5ever parse error (`html_sink.rs:153-155`) and prints each one with `println!("ERROR: {error}")` in `TreeSink::finish` (`:147-151`). That goes to fd 1, from library code, on whatever thread is parsing.

A document without a doctype always produces html5ever's "Unexpected token" (`html5ever-0.35.0/src/tree_builder/rules.rs:48-58`, `mod.rs:617-626`; the message is generic because `exact_errors: false` is set at `html_sink.rs:91`). fulgur builds every `@page` margin-box document from a wrapper without a doctype (`render.rs:4979`, `:5022`, `:5105`). As a result:

- `bench/corpus/b200-items.html` prints 416 lines and `02-long` prints 9.
- Any user template without a doctype prints at least one line.

Our CLI does not isolate fd 1. fulgur's `StdoutIsolator` (`fulgur-cli/src/main.rs:30-110`, used at `:624`) lives only in fulgur's CLI. Our `main.rs:108` only sets up `env_logger` on stderr. So CLI stdout is the library's fd 1. Library callers (the planned HTTP service, bindings) get this noise too, and fulgur's `CLAUDE.md` forbids library crates from touching fd 1. A caller that streams the PDF to stdout would get a corrupted stream.

`crates/htmltopdf-cli/tests/stdout_clean.rs` passes today only because its document has a doctype and no margin boxes (`:29`).

## 2. Spec

No `css-spec-researcher` output exists. The references below are cited from memory and their section numbers are **unverified**.

- **HTML LS §13.2.2 (parse errors).** Recovery from a parse error is fully defined, and reporting parse errors is optional for user agents. Browsers do not show them to the reader. They are diagnostics, not content.
- **HTML LS §13.2.6.4.1 ("initial" insertion mode).** A missing doctype is a parse error and sets quirks mode.
- **HTML LS §13.2.6.4.7 ("in body", start tag `table`).** Only outside quirks mode does a `<table>` start tag close an open `<p>`. html5ever implements this at `rules.rs:608-611`, and it is the only place where html5ever's tree building depends on quirks mode.
- **Quirks Mode LS.** It covers the CSS effects of quirks mode, but Blitz always styles in no-quirks mode: `Stylist::new(device, QuirksMode::NoQuirks)` (`document.rs:293`), `quirks_mode()` returns `NoQuirks` (`stylo.rs:168-169`), and the sink stores html5ever's mode but nothing reads it (`html_sink.rs:260-262`).

## 3. Where it belongs, and the decision

**Engine behaviour belongs upstream.** The fix goes in `blitz-html`. We must patch it, because the bug is a `println!` inside Blitz and no caller can turn it off without the racy fd juggling that fulgur already removed.

- `blitz-html` is a member of our Blitz fork workspace (`/home/user/blitz/Cargo.toml:6`, version `0.2.0` from `:24`).
- Its source is identical to crates.io 0.2.0 (`lib.rs`, `html_document.rs` and `html_sink.rs` compared). Our build uses the crates.io copy today (`Cargo.lock:231-234`).

**Change (a), taken.** Three edits:

- `packages/blitz-html/Cargo.toml`: add `log = "0.4"`. `log` 0.4.34 is already in our lock, so no new package.
- `TreeSink::finish`:

  ```rust
  for e in self.errors.borrow().iter() { log::debug!("html parse error: {e}") }
  ```

- Our workspace `Cargo.toml` `[patch.crates-io]`:

  ```toml
  blitz-html = { git = "https://github.com/subapavel/blitz", branch = "htmltopdf/blitz-dom-0.2.4" }
  ```

  The version stays 0.2.0, otherwise Cargo ignores the patch.

**Log level: `debug` for every parse error, not `warn`.** Reasons:

1. With `exact_errors: false` the messages carry no token and no position ("Unexpected token"), so a warning would not tell the author what to fix.
2. Recovery is defined by the spec and matches Chrome.
3. A missing doctype changes nothing we render, because Blitz styles in no-quirks mode (§2). The only effect is the `<p><table>` tree rule.
4. At `warn`, our CLI's default filter (`main.rs:108`) would print 416 lines on stderr for `b200-items`.

The errors stay reachable with `RUST_LOG=blitz_html=debug`.

**Other fd-1 writes on render paths, same Blitz commit series.** We must cover these too, because the invariant is "the library writes nothing to fd 1", not one call site. They move to `log` (blitz-dom gains `log = "0.4"`):

| Site | What it prints | New call |
|---|---|---|
| `blitz-dom` `layout/construct.rs:192-194` | inline `<svg>` that usvg rejects; prints the whole outer HTML, and the SVG is dropped silently | `log::warn!` with the node id and the error; outer HTML at `debug` |
| `layout/table.rs:782` | table descendant with no styles | `log::debug!` |
| `layout/mod.rs:146-150` | text node laid out on its own (`println!` + `dbg!`) | `log::warn!` |
| `document.rs:907` | "No DOM" | `log::debug!` |

The SVG case gets `warn` because content disappears (standing decision: warn by default).

These stay as they are:

- `print_tree`, `debug.rs` and `util.rs:46-98`: debug APIs that the caller invokes explicitly.
- `events/*` and `document.rs:957` (`hit`): never run during a render.

**Change (b), a doctype in the margin wrapper: not done.** After (a) it removes nothing more from stdout. It only changes the tree in quirks mode, and that is wrong here:

- A running element copied from a document without a doctype can hold `<p>…<table>`, because that document parsed in quirks mode.
- Today the quirks-mode margin parse reproduces the body's tree. With a doctype, the margin copy would differ from the body.
- So (b) is not byte-identical by construction, and it is only worth doing if parse errors were ever logged at `warn`.

**fulgur fork: no change.** The "Stdout hygiene" comment (`blitz_adapter.rs:11-21`) and the `CLAUDE.md` gotcha describe crates.io `blitz-html`, which still prints. Keeping the fork diff at zero avoids rebase cost.

## 4. Acceptance

**Red tests first.** Each is red on main.

1. `crates/htmltopdf-cli/tests/stdout_clean.rs`, new test `no_doctype_and_margin_boxes_write_nothing_to_stdout`.
   - Input: a document without a doctype, 3 or more pages, `@page { @bottom-right { content: "Strana " counter(page) " / " counter(pages) } }`, an inline `<svg width="0" height="0">` (the usvg rejection trigger is **unverified**; step 0 finds one), rendered with `-o file`.
   - Assertions: stdout is empty, the output file starts with `%PDF-`, and stderr has no "Unexpected token" (the error is not at `warn`).
   - Red today: stdout holds `ERROR: Unexpected token` lines.
   - The CLI is a library caller with no fd isolation, so this checks the library's fd 1 with no new dependencies.
2. Same file, `parse_errors_are_visible_at_debug`: the same run with `RUST_LOG=blitz_html=debug`. Stdout is empty and stderr contains "Unexpected token". Red today, because the line goes to stdout.
3. `crates/htmltopdf/tests/parse_error_log.rs` (one test, because the logger is global to the process; uses the `Capture` pattern of `page_overflow_warning.rs`).
   - `Renderer::render` of a document without a doctype gives at least one `Debug` record with target `blitz_html::*` containing "Unexpected token", and no record from `blitz_html` at `Warn` or above.
   - The SVG case gives one `Warn` record from `blitz_dom`.

**Bytes.**

- Fixtures 01-20 and all 7 documents in `bench/corpus/` must be `cmp`-identical to main's binary at `RAYON_NUM_THREADS=1` and `=4`. Logging does not touch layout.
- `compare.py`'s `summary.json` must be byte-identical: the pixel-diff change is 0 for every fixture.
- Look at `out/compare/02-long-p3.png` once: the footer must be present.
- Record the stdout line count: 416 → 0 on `b200-items`, 9 → 0 on 02.

**Fork tests.**

- `blitz-html` lib test `parse_errors_go_to_log`: a capture logger plus `HtmlDocument::from_html("<p>x</p>")`.
- `blitz-dom` lib test for the SVG warning.
- `cargo test -p blitz-html -p blitz-dom --lib`, with and without `parallel-construct`.

**Other checks.**

- `cargo tree -d` shows one `blitz-dom`, and `Cargo.lock` shows `blitz-html` from the git source.
- `bench.py --check` is ok.
- `vrt-validator` runs. `pdf-reviewer` is not required, because the writer, fonts and metadata are untouched.

## 5. Steps

Each step takes one day or less and can be tested on its own.

0. **Baseline (½ day).**
   - Build main's binary and record the `cmp` references (fixtures and corpus, 1 and 4 threads) and the stdout line counts.
   - `diff -r` the fork's `packages/blitz-html/src` against the crates.io 0.2.0 source.
   - Find an inline SVG that usvg rejects. If there is none, drop the SVG case and record it as a finding.
1. **Red tests (½ day).** Write the §4 tests 1-3 and see them fail for the stated reason.
2. **Blitz fork, `blitz-html` (½ day).** The `log` dependency, `finish` → `log::debug!`, the unit test. Push.
3. **Blitz fork, `blitz-dom` (½ day).** The four sites in §3, the unit test, the lib tests with both feature sets. Push.
4. **Ours (1 day).**
   - Add the `[patch.crates-io]` line, then `cargo update -p blitz-html -p blitz-dom`.
   - Get the tests green, run the `cmp` matrix, `compare.py` and the bench check.
   - Docs: `docs/upstream.md` (the F100 row becomes **carried**, and the Blitz row in "Our patches" gets `blitz-html 0.2.0`), mark the F100 entry done, write §7 of this plan.
   - Run `vrt-validator`, then merge.

## 6. Risks and non-goals

**Risks.**

- **Cargo silently ignores a patch whose version differs.** Tests 1-3 only go green if the patch is in effect, and step 4 checks the lock source.
- **A second `blitz-dom` in the graph.** `blitz-html`'s path dependency inside the git checkout should resolve to the same package as the patched `blitz-dom`. `cargo tree -d` checks it.
- **`debug` hides malformed HTML from authors.** That is accepted for the reasons in §3. A per-render report API can come later.
- **The guard only covers the paths the test documents run.** A new `println!` elsewhere would pass unnoticed until a fixture hits it.

**Non-goals.**

- Change (b).
- `StdoutIsolator` in our CLI: it is not needed once the library is clean, and it would hide regressions from test 1.
- `eprintln!` to stderr (fulgur `link.rs:91`, blitz-dom `node/element.rs:305`, `:317`, `:348`). fd 2 is not forbidden; record it as a follow-up finding.
- A `--verbose` CLI flag.
- Quirks-mode support in Blitz.
- Editing fulgur's docs.

## 7. Result

Done on branch `f100-stdout-parse-errors`. Output is unchanged: the library now writes nothing to fd 1 on any fixture or corpus document.

**Commits.**

- Blitz fork `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`: `c1c708c` (`blitz-html`: `log` dependency, `finish` -> `log::debug!`, lib test `parse_errors_go_to_log`) and `d5d60b7` (`blitz-dom`: `log` dependency, the four sites of §3, lib test `a_rejected_inline_svg_is_a_warning`). Base `a4b7624`. The Blitz `Cargo.lock` that local runs rewrite was not committed.
- Ours: red tests, then `[patch.crates-io] blitz-html` with the lock change, then docs.

**Red / green.** All three tests were red on main for the stated reason and are green now.

| Test | Main | Now |
|---|---|---|
| `stdout_clean.rs` `no_doctype_and_margin_boxes_write_nothing_to_stdout` | stdout holds `ERROR: Unexpected token` lines and the SVG dump (`286 SVG parse failed` + the markup) | ok |
| `stdout_clean.rs` `parse_errors_are_visible_at_debug` | same stdout | ok (stderr has "Unexpected token" with `RUST_LOG=blitz_html=debug`) |
| `parse_error_log.rs` | no record at all | ok: `Debug` record from `blitz_html`, none at `Warn`, one `Warn` from `blitz_dom` for the SVG |

Fork tests: `cargo test -p blitz-dom -p blitz-html --lib` 75 + 2 passed, and 77 + 2 with `--features blitz-dom/parallel-construct`.

**stdout lines** (`htmltopdf render`, bench flags, `RAYON_NUM_THREADS` 1 and 4, same counts):

| Document | Main `f821b67` | Now |
|---|---|---|
| `b200-items` | 437 | 0 |
| `02-long` | 9 | 0 |
| `17-running-footer`, `18-a5-receipt` | 2 each | 0 |
| all other 23 documents | 0 | 0 |

stderr is one line per document on both binaries (the timing line; `slow-breaks` has its `@layer` warning, two lines): no new noise. The plan said 416 lines for `b200-items`; main's binary prints 437 (not investigated why the earlier number differs; the PDF is unchanged either way).

**Bytes.** 27 documents (fixtures 01-20, the 7 corpus documents) x 1 and 4 threads = 54 renders: 54 of 54 sha256-identical to `/tmp/claude-0/htmltopdf-main-f821b67`. `compare.py`'s `summary.json` is byte-identical to main's (every fixture's pixel diff is unchanged). `out/compare/02-long-p3.png` was looked at: the footer "Strana 3 / 3" is present.

**Level decisions.**

- Parse errors: `debug` (§3).
- Text node laid out on its own (`layout/mod.rs`): `warn`. The site did not fire on any fixture or corpus document (stdout would have shown "Tried to lay out"; it did not), so the CLI prints nothing new. The text of such a node is hidden, which is content loss.
- Rejected inline SVG: `warn` with node id and usvg's error (`InvalidSize` for `width="0" height="0"`, found in step 0); the markup at `debug`.
- Table descendant without styles, "No DOM": `debug`.

**Other checks.** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --release --locked` (572 tests) pass; `cargo tree -d` shows no second `blitz-dom` or `blitz-html`, and `Cargo.lock` has `blitz-html` from the git source. `bench.py --check` ok (`b200-items` 1611 ms, `b200-mixed` 1117 ms, the 1 s goal is not met on this machine, as before). `Cargo.lock` changed in blitz-html's source line, the `blitz-dom` pin and the `log` edges only; the `skrifa` rewrite that `cargo update` made was reverted by hand.

**Deviations.**

- 437 lines instead of 416 (above).
- Step 0's `diff -r` of the fork's `blitz-html/src` against crates.io 0.2.0: identical.
- The blitz-dom SVG test builds the tree with the mutator (blitz-dom does not depend on `blitz-html`), and the blitz-html test uses `log::set_logger` with a static (the `std` feature of `log` is off in the fork's lock).
- Not done: `vrt-validator` (run by the caller), `pdf-reviewer` (not required), `eprintln!` sites (F151).
- Not verified: Blitz `main` may have changed these sites; the cost of rebasing the two fork commits onto a Blitz bump is not measured.

