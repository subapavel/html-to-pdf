# F177b — usvg reads host files for the non-`data:` image hrefs of an SVG

Date: 2026-10-26. Finding: F177b (`2026-10-15-m0-findings.md`). Fix on `subapavel/fulgur` `htmltopdf/srgb-colours`;
the strict-mode mapping is ours. No Blitz change.

## 1. Problem

usvg 0.45.1's `default_string_resolver` (`usvg-0.45.1/src/parser/image.rs`) treats every non-`data:` `<image href>` as
a path: `Path::exists`, then `std::fs::read`, `resources_dir` `None` (relative to the process's working directory, or
absolute). F177's `SvgImageChecks::resolver` (`fulgur/src/svg_images.rs`) wraps that resolver, so what it reads is
validated but still comes from the host disk: `<svg><image href="logo.png">` draws `./logo.png` if the CLI runs where
that file exists; `href="/etc/…/x.png"` embeds any image file the process can read; `href="/dev/zero"` is an unbounded
read (killed at 8-12 GB, F177 vrt-validator).

A second leak F177 did not list: `fulgur::image::validate_detailed` parses a bundled SVG with
`usvg::Options::default()`, and our `Renderer::check_images` calls it before the engine runs, so `<img src="card.svg">`
with `/dev/zero` inside hangs there. The same default options are the `None` fallback in `blitz_adapter.rs`
(`seed_image_natural_sizes`), `convert/style/background.rs` and `convert/list_marker.rs`.

This breaks AGENTS.md goal 3 (only the input and the bundle decide the output; nothing is read from the host).

## 2. Spec (from memory, unverified; step 0 checks Chrome)

SVG 2 §5.7: `href` is a URL reference; in an inline `<svg>` it is resolved like `<img src>` (our fetch is a lookup by
name in the bundle, F32a). SVG Integration §3: an SVG used as an image is processed in secure (static) mode — no
external resources; `data:` URLs are not external (usvg already does this for an SVG nested in `<image>`). HTML
§4.8.4.1: an image that cannot be obtained is broken, the rest renders (F32a: warn, strict refuses). From the code:
`image.rs:90` is usvg's only filesystem access; an SVG's `<style>` feeds only presentation attributes (`href` is not
one) and simplecss has no at-rules, so CSS cannot read files; `feImage`, `<pattern>`, `<mask>`, `<clipPath>` images go
through the same resolver.

## 3. Where it belongs

fulgur's resolver, installed at every parse site by F177. Blitz's `net.rs` `parse_svg(&bytes, None)` runs only for
files a `FulgurNetProvider` serves from `base_path`, which our crate never sets (F177d, filed).

| Where the SVG comes from | Non-`data:` href | Finding |
|---|---|---|
| Inline `<svg>` | `AssetBundle::get_image(href)` with the `<img src>` rule (exact key, leading `./` dropped, empty = missing); found bytes go through usvg's `default_data_resolver()` (MIME sniffed) and F177's check | not bundled → `NotBundled` |
| `<img src>`, `background-image`, `list-style-image` | never loaded (secure mode), bundled or not | `External` |
| margin-box documents | inline rule with no bundle: every href `NotBundled`, warned (like a margin-box `<img>`) | `NotBundled` |

No case touches the filesystem; `/dev/zero`, `file:` and `http:` hrefs are names that are not in the bundle.

fulgur `svg_images.rs`: in `resolver` drop the default string resolver, call `resolve_href(&site, href, opts)`; add
`images: Option<Arc<AssetBundle>>` to `SvgImageChecks`, `cause: SvgImageCause` to `Finding` and `DroppedSvgImage`;
`pub enum SvgImageCause { Undecodable, NotBundled, External }`, `SvgImageChecks::new(images)`, `pub fn
offline_options() -> usvg::Options<'static>` (both resolvers fs-free, `data:` only, no checks). `image.rs`
`validate_detailed` and the three `None` fallbacks use `offline_options()`. `engine.rs` keeps the bundle as an `Arc`
(no per-render clone) and passes it to `SvgImageChecks::new`.

Ours (`lib.rs`): `ImageProblemKind::UnregisteredInSvg { image, why }` (`src` names the SVG; Display `logo.png in <svg
id="s"> is not a bundled image` / `… is never loaded: an SVG used as an image loads no external files; embed it as a
data: URL`); `run()` maps `Undecodable` to `InSvg` and the other two causes to the new variant. Strict refuses both.

## 4. Acceptance

`crates/htmltopdf/tests/svg_image_offline.rs` (helpers as in `svg_image_robustness.rs`; cases that need a working
directory or could hang run as child processes whose working directory is a temp dir holding a red `logo.png`):

1. relative, absolute, `file:` and `http:` hrefs in an inline `<svg>` are never read: `Ok`, 0 image XObjects, rect
   painted, one warning per href naming the SVG and the href (red: 1 XObject each).
2. `/dev/zero` in an inline `<svg>`, an `<img src="card.svg">` and a `background-image`: the child exits 0 within 2 s
   (red: hangs).
3. a bundled `logo.png` / `./logo.png` in an inline `<image>` (and `feImage`) is drawn: 1 XObject, red pixels, no
   warning, strict `Ok`.
4. a bundled `card.svg` with `href="logo.png"` (also bundled) as `<img>`, background, list marker: 0 XObjects, one
   `External` warning each; its `data:` twin draws.
5. strict refuses unbundled and external hrefs with `UnregisteredInSvg`, document order; non-strict `Ok`.
6. guards: nested `data:image/svg+xml` stays unloaded; an SVG `<style>` with `@import` / `url(/…/logo.png)` renders
   byte-identically to the SVG without it; determinism.

Fork tests: `svg_images.rs` unit tests per cause, `offline_options` loads `data:` only; `validate_detailed` of an SVG
with `/dev/zero` returns in < 1 s; `render_smoke` one per cause; VRT unchanged. Fixtures 01-25 (no `<image>`,
`xlink:href`, `feImage` or `.svg`) and F177's valid-raster probes byte-identical; pixel-diff change 0.

## 5. Steps

0. Probes and Chrome reference in `out/scratch/f177b/` (Chrome draws an inline `<image href="logo.png">` next to the
   HTML; draws nothing for it inside `<img src="card.svg">` but draws a `data:` image there); record in §7. 1. Ours:
   tests 1-6. 2. fulgur: `offline_options()` at `validate_detailed` and the three fallbacks, unit test. 3. fulgur:
   `SvgImageCause`, `resolve_href`, `SvgImageChecks::new(images)`, the `Arc` bundle in `Engine`, tests. 4. Ours: lock
   bump, `UnregisteredInSvg`, mapping; gates, `vrt-validator`; ADR 0006 "SVG hrefs: inline by bundle name, SVG-as-image
   never external"; `docs/upstream.md`, findings (F177b done, F177d filed), §7.

## 6. Risks and non-goals

- A document that relied on a host-relative or absolute href loses that image, with a warning (intended).
- Strict is stricter than Chrome: `--strict-images` refuses an SVG-as-image whose external href Chrome silently omits
  (design-tool exports with stale `xlink:href`). Strict is opt-in.
- A new `ImageProblemKind` variant breaks exhaustive matches (pre-1.0, unpublished).
- Non-goals: `<base href>` and name matching beyond the exact key (F32f); F177c; SVG text and fonts (F152); F177d
  (a fulgur user setting `base_path` gets SVGs parsed by Blitz `net.rs` with usvg's default resolver; unreachable from
  our crate).
