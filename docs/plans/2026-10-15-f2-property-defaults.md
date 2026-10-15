# F2 — Apply `@property` initial values (Tailwind v4 borders, gradients, shadows)

Date: 2026-10-15. Finding: F2 in `2026-10-15-m0-findings.md`.

## 1. Problem

Tailwind v4 gives its `--tw-*` custom properties their defaults through `@property` rules (end of
`fixtures/tailwind/tailwind.css`), e.g. `@property --tw-border-style { syntax:"*"; inherits:false; initial-value:solid }`.
`.border` is then `border-style: var(--tw-border-style); border-width: 1px`. Our engine (Stylo via Blitz) does
not apply registered initial values, and Tailwind's own fallback block (`@layer properties { @supports (… -moz-orient …
and not (color: rgb(from red r g b))) { … } }`) is skipped because Stylo supports relative colour syntax. So
`--tw-border-style` is unset → `border-style` is invalid at computed-value time → `none`: **no Tailwind border renders
in any fixture**. The same mechanism removes gradients (`--tw-gradient-from/to/…-position`), shadows and rings
(`--tw-shadow`, `--tw-ring-offset-width`, …), and breaks the non-inheritance of `--tw-leading`, `--tw-font-weight`,
`--tw-tracking` (a child inherits its parent's `--tw-leading` instead of falling back to its own `text-*` line height).

Visible today: 01's header rule, 03's cell and card borders, 04's `border-l-4` bar, 05's card borders and shadows.
(Borders on `<tr>` — 01's and 02's row rules — paint only in the collapsing border model, Blitz #386; not F2.)

## 2. Spec

- css-properties-values-api-1 §2 (`@property`): `syntax` and `inherits` are required; `initial-value` is required unless
  the syntax is the universal `*`; an invalid rule is ignored; for several valid rules with the same name the last
  one wins. A registered property with `inherits: false` takes its initial value on every element where it is not
  set (it does not inherit); with `inherits: true` it inherits from the parent, the root gets the initial value.
  A universal-syntax property without `initial-value` has the guaranteed-invalid value as initial value.
- css-properties-values-api-1 §2.5 / css-variables-1 §2: a registered typed property substitutes its *computed*
  value — `<length>` `0` substitutes as `0px` (so `calc(1px + var(--tw-ring-offset-width))` stays valid).
- css-variables-1 §2.2: `--x: initial` on an unregistered custom property sets the guaranteed-invalid value, so
  `var(--x, fallback)` uses the fallback.
- css-cascade-5 §6.4: normal declarations in cascade layers lose to unlayered ones; among layers, the first
  declared layer has the lowest priority; layer order is fixed by first appearance across all style sheets.

Confirmed by the css-spec-researcher report (PR description): TR 2024-03-26 §3–§3.3 for the descriptors, §2.4/§2.7
for computed-value substitution, css-variables-1 §2.2/§3, css-cascade-5 §6.1/§6.4.2/§6.4.3. WPT:
`css/css-properties-values-api/determine-registration.html`, `registered-property-initial.html`, `conditional-rules.html`.

## 3. Where it belongs

Root fix is upstream: Stylo 0.8.0 compiles the whole `@property` pipeline for servo, but the parser is gated by the
static pref `layout.css.properties-and-values.enabled`, which `stylo_static_prefs` 0.8.0 does not list (→ false)
(`stylo-0.8.0/src/stylesheets/rule_parser.rs:752`). Tracked as F2b in `docs/upstream.md`. Local now, in
`crates/htmltopdf` only (the finding chose option (a)):

- New `crates/htmltopdf/src/property.rs`:
  - `scan_property_rules(css) -> Vec<PropertyRule>` — cssparser token walk; top-level and inside `@layer` blocks
    (rules inside `@media`/`@supports` are conditional and are not polyfilled).
  - `PropertyRule { name, syntax, inherits, initial }`; validity per §2; later rule of the same name replaces earlier.
  - `prelude(rules) -> Option<String>`:
    ```css
    @layer htmltopdf-property-defaults {
      *, ::before, ::after { --a: <initial>; --b: initial; }   /* inherits: false */
      :root { --c: <initial>; }                              /* inherits: true  */
    }
    ```
    `<length>` / `<length-percentage>` syntax with a unitless `0` initial value emits `0px` (the only computed-value
    normalisation needed; typed computation is otherwise a non-goal).
- `lib.rs`: `RendererBuilder::css()` collects rules; `build()` inserts the prelude at index 0 of the bundled CSS, so
  `htmltopdf-property-defaults` is the first layer declared in that sheet → lowest priority. Every Tailwind layer
  (`theme`, `base`, `utilities`) and any unlayered author rule override it, exactly like a real initial value.
- No prelude when no `@property` rule was bundled → output for non-Tailwind CSS is byte-identical to today.
- `@property` in the document's own `<style>`: **not** polyfilled. fulgur appends the bundled CSS *after* the
  document's `<style>` elements, so a prelude layer would be declared after the document's layers and override them
  (e.g. win over `utilities`). We log a warning recommending to pass that stylesheet via `css()` / `--css` instead.

## 4. Acceptance

- **A1 `tests/property_defaults.rs`** (rasterises with `pdftoppm`, fails loudly if missing), CSS with pure sRGB colours
  so F1 does not interfere:
  1. `border-style: var(--bs)` with `@property --bs { initial-value: solid }` → red border pixels present (red before).
  2. A layered utility setting `--bs: none` on the element → no red pixels (author layer beats the default).
  3. `inherits: false`, no initial value: parent sets `--c: blue`, child paints `var(--c, red)` → red, no blue.
  4. `<length>` initial `0` inside `calc(4px + var(--w))` box-shadow → shadow pixels present.
  5. `inherits: true` initial value read by a descendant → present.
  6. Invalid rules (missing `inherits`; non-universal syntax without `initial-value`) → ignored.
  Unit tests in `property.rs` for parsing, validity, last-wins and prelude text.
- **A2 fixtures**: `cargo test --release --workspace` green, render deterministic, page counts unchanged (02 stays in
  `PAGE_COUNT_MAY_DIFFER`). In `out/compare/*.png` the Tailwind borders on block and cell elements appear in 01–04,
  and 05 shows its card borders and shadows (its gradients: see Outcome / F8). Colours stay wrong until F1 (oklch), so per-fixture diffs may *grow* where a
  previously missing border/gradient now renders in a wrong colour — allowed by merge policy rule 3; the PR names each
  fixture and number. Thresholds are lowered where the number improves.
- **A3** `scripts/compare.py` numbers before/after in the PR.

## 5. Steps

1. Probe (`out/scratch/f2-probe.html`): confirm no borders today, and that a layered `*` default in the bundled CSS
   is overridden by Tailwind's utilities layer in Stylo.
2. `property.rs` + unit tests (red → green).
3. Wire into `RendererBuilder`; `tests/property_defaults.rs` (red → green).
4. Render fixtures, compare, look at PNGs, update thresholds/docs (`m0-findings` status, `upstream.md`).

## Outcome (2026-10-15)

- Probe confirmed every mechanism in Stylo (layered `*` default overridden by later layers and unlayered rules,
  `--x: initial` falls back, `0px` keeps `calc()` valid) and that fulgur appends bundled CSS after document `<style>`.
- Fixtures: borders, dividers, card borders/shadows render; render time unchanged (02-long 55 ms).
- **Gradients do not come back**: Tailwind emits `linear-gradient(to … in oklab, …)` and the engine drops any gradient
  with a colour-interpolation method. Not an `@property` problem → new finding F8.
- 03-logo-qr grows past its threshold (every cell's newly visible border is painted in F1's wrong colour and doubled
  by Blitz #386). With the palette converted to sRGB (measurement only) 03 is 8.9 % — under the threshold — so
  this PR waits for F1 instead of raising the threshold.
- vrt-validator: no PDF defects, determinism holds, page counts unchanged. A 215-page table renders in 0.87 s with
  the Tailwind defaults vs 0.77 s without (34 custom properties on every element) — inside the 1 s budget, watch it.
  05's PDF grows 52 → 170 KB: the now-visible blurred shadows are emitted as shading patterns + soft masks.
  Fixed from its findings: comments inside descriptors, `!important` in a descriptor (now invalid), and the
  document warning now matches `@property --x` only; a second warning fires when the document declares `@layer`
  while defaults are active (its layers rank below ours — see Risks).
- Not polyfilled (non-goals confirmed by research): `@property` inside `@media`/`@supports`; `unset`/`initial`/
  wrong-type author values on a registered property (real registration → initial value, emulation → inherit /
  guaranteed-invalid / kept); `::backdrop`/`::marker` (irrelevant to print).

## 6. Risks and non-goals

- Non-goal: typed `@property` semantics (invalid-at-computed-value → initial for typed syntaxes, animation, computed
  colour/length normalisation beyond `0` → `0px`).
- Risk: a document `<style>` that declares a layer *before* the bundled CSS (layer order is first-appearance) puts that
  layer below our defaults layer; its `--tw-*` declarations would then lose to the default. Same exposure as
  Tailwind's own `@layer properties` fallback in browsers; not seen in fixtures.
- Risk: line heights change where `--tw-leading` leaked by inheritance — moves towards Chrome, may move pagination;
  any page-count change outside `PAGE_COUNT_MAY_DIFFER` blocks the merge.
- Colours of the newly visible borders/gradients are wrong until F1; that is F1's acceptance, not this one.
