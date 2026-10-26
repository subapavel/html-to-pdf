# F177 — rasters inside an SVG bypass the image checks

Date: 2026-10-26. Finding: F177 (`2026-10-15-m0-findings.md`). Fixes go on `subapavel/fulgur` `htmltopdf/srgb-colours`
and `subapavel/blitz` `htmltopdf/blitz-dom-0.2.4`; the strict-mode mapping is ours.

## 1. Problem

Found by the F99e pdf-reviewer, on main too; no fixture has an SVG `<image>`. An inline `<svg>` with `<image
href="data:image/png;base64,…">` whose IHDR claims 100000 x 100000 RGBA aborts the process ("memory allocation of
40000000000 bytes failed", exit 134; `catch_unwind` in `Renderer::run` cannot catch it). A truncated embedded PNG fails
the whole render (`PDF generation error: Image(…, "unexpected end of file")`, exit 1). The same image as a top-level
`<img>` is skipped with a warning, or refused with `--strict-images` (F32a).

Cause: usvg's default data resolver (`usvg-0.45.1/src/parser/image.rs`) only reads the MIME type or magic bytes, and
`kind.actual_size()` only the header; krilla-svg calls `Image::from_png/jpeg/gif/webp` directly
(`krilla-svg-0.7.0/src/image.rs`); krilla reads metadata only and decodes the pixels in `Document::finish()` — since
F99e on a rayon worker, where a panic or a failed allocation aborts. F32a's `validate` / `MAX_IMAGE_PIXELS`
(`fulgur/src/image.rs`) is never consulted. The same holds for `<img src="x.svg">` (`seed_image_natural_sizes`), SVG
`background-image` (`convert/style/background.rs`), SVG `list-style-image` (`convert/list_marker.rs`), and `<image>`
inside `<pattern>`, `<mask>`, `<clipPath>` and `feImage` (also through `get_href_data`). A filtered group is rasterised
by resvg on the main thread, which decodes its images there too.

Resolution facts: an SVG inside an SVG's `<image>` is parsed with both resolvers returning `None` (`load_sub_svg`), so
its own images are never loaded. Non-`data:` hrefs go to `default_string_resolver`, which reads the path from the host
disk, relative to the process's working directory or absolute (F177b).

## 2. Spec (from memory, unverified)

SVG 2 §5.7: an `<image>` whose resource cannot be loaded or decoded is in error and not rendered; the rest of the
document is. SVG Integration §3: an SVG referenced from `<image>` is processed in secure static mode (no external
resources), matching usvg dropping nested images. HTML §4.8.4.1: an image corrupted in some fatal way is broken (F32a's
basis). Chrome (to be confirmed in step 0) draws the SVG without the broken `<image>`.

## 3. Where it belongs

The check goes where usvg turns an href into an `ImageKind`: a custom `usvg::ImageHrefResolver` that wraps usvg's
`default_data_resolver()` / `default_string_resolver()`. A refused raster never enters the tree (`get_href_data(..)?`
skips the element as it skips an href that does not resolve); a valid one is returned as the same `ImageKind` with
the same bytes, so the tree and the PDF are byte-identical. A walk over the finished tree is rejected: usvg 0.45 cannot
remove nodes and an inline `<svg>` has no source bytes in fulgur.

| Where | Change |
|---|---|
| fulgur `image.rs` | `pub fn validate_svg_raster(kind: &usvg::ImageKind) -> Result<Option<String>, ImageError>`: PNG / GIF with `validate_png` / `validate_gif`; JPEG with `validate_jpeg` (`Some(why)` = damaged but embedded, F32a M1); WebP with a new `validate_webp` (`image-webp` at krilla's locked version: `check_size`, then `read_image`); SVG `Ok(None)`. |
| fulgur `svg_images.rs` (new) | `DroppedSvgImage { svg, image, reason }`; `SvgSite { Inline(usize), Img(String), Background(String), ListMarker(String) }`; `SvgImageChecks` (one per `Engine::render`, `Arc`): dropped and damaged lists, a verdict cache keyed by the bytes (`BTreeMap<Arc<Vec<u8>>, Verdict>`: usvg decodes a `data:` URL into a fresh `Vec` on every parse), `resolver(site)`, `options(site)`, `take(doc)` sorted by site then href (thread-independent order); an inline SVG named `<svg id="…">` or the k-th `<svg>`; a `data:` image named `data:image/png (N bytes)`. |
| fulgur parse sites | `seed_image_natural_sizes`, `background.rs`, `list_marker.rs` use `checks.options(site)` instead of `usvg::Options::default()`. |
| Blitz fork | `config.rs`: `pub svg_image_resolver: Option<SvgImageResolverFactory>` (`Arc<dyn Fn(usize) -> usvg::ImageHrefResolver<'static> + Send + Sync>`), stored on `BaseDocument`; `util.rs` `parse_svg(source, resolver)`, `None` = upstream behaviour; `construct.rs` passes `factory(container_node_id)` (the only place the inline tree is built). |
| fulgur engine | `blitz_adapter.rs` sets the factory in both `DocumentConfig`s (margin documents covered); after `dom_to_drawables` (the `dropped_text` pattern, pass 1 only): `EngineBuilder::strict_images(bool)`; strict → `Error::SvgImages(Vec<DroppedSvgImage>)`, otherwise one `log::warn!` per image (capped), damaged JPEGs warned in both modes. |
| ours `lib.rs` | `engine()` sets `.strict_images(self.strict_images)`; `fulgur::Error::SvgImages` maps to `Error::Images` with a new `ImageProblemKind::InSvg { image, why }` (`src` names the SVG). |

The resolver runs inside usvg's parse, on the thread that builds the document, before krilla: no unvalidated raster
reaches a rayon worker.

## 4. Acceptance

New test binary `crates/htmltopdf/tests/svg_image_robustness.rs` (separate, so a red abort does not kill F32a's tests;
helpers from `image_robustness.rs`, moved to `tests/common/` if needed). A 100 x 50 SVG with a blue rect and the
`<image>`, plus a marker div:

1. `huge_png_header_in_inline_svg_does_not_abort` (re-runs itself as a child process): exit 0 within 2 s, 0 image
   XObjects, rect and marker painted, one warning naming the SVG and "pixel limit". Red: SIGABRT (134).
2. `truncated_png_gif_webp_in_svg_are_dropped_with_a_warning` (3 formats x inline `<svg>`, `<img src="logo.svg">`,
   `background-image`): `Ok`, 0 XObjects, rect painted, one warning naming the SVG and `data:image/…`. Red (`Err`).
3. `strict_images_refuses_a_broken_raster_in_svg`: `Error::Images` with `InSvg`, document order; non-strict `Ok`. Red.
4. `truncated_jpeg_in_svg_is_embedded_with_a_warning` (F32a M1): 1 XObject, `Ok` in strict mode, one "embedded although
   damaged" warning.
5. `broken_rasters_in_pattern_mask_and_feimage_are_dropped`: `Ok`, no abort (`feImage` in a child process). Red.
6. `nested_svg_image_is_never_loaded`: guard, `Ok`, no warning.
7. `valid_rasters_in_svg_are_drawn` (PNG, JPEG, GIF, WebP x 3 sites): 1 XObject each, pixels inside the rect ±1 px,
   no warning. Guard.
8. `render_with_a_broken_svg_raster_is_deterministic`.

Fork tests: `validate_svg_raster` unit tests (valid, truncated, 100000² without allocating, WebP over the limit); a
Blitz construct test (the factory's resolver is used; a refused `<image>` leaves no image node; `None` unchanged);
`render_smoke` per parse site and strict `Error::SvgImages`; `fulgur-vrt` unchanged. Fixtures 01-25 and valid-raster
probes byte-identical to main; pixel-diff change 0; no new fixture.

## 5. Steps

0. Probes in `out/scratch/f177/` (cases 1-8, `href="logo.png"` relative and absolute): exit codes on main, which hrefs
   usvg resolves; Chrome reference for 2. 1. Ours: tests 1-8, red where marked. 2. fulgur `image.rs`:
   `validate_svg_raster`, `validate_webp`, `image-webp` dependency, unit tests. 3. Blitz fork: the factory and
   `parse_svg(…, resolver)`, construct test, `cargo test -p blitz-dom --lib` with and without `parallel-construct`.
   4. fulgur: `svg_images.rs`, the three parse sites, the factory in both configs, engine reporting, `strict_images`,
   `Error::SvgImages`, `render_smoke`, VRT. 5. Ours: lock bump, `InSvg`, error mapping, gates, `pdf-reviewer`,
   `vrt-validator`, `docs/upstream.md`, findings (F177 done, F177b filed), §7.

## 6. Risks and non-goals

- Decoding twice (validate once per render, byte-keyed cache; krilla once): about 3-4 ms per 2000 x 1000 PNG, under 0.2
  ms for a 400 x 200 logo; peak memory the larger of the two, not the sum. Sharing would need a krilla-svg fork.
- A `validate` verdict that differs from krilla's turns back into an abort or a failed document: same decoder crates at
  krilla's locked versions; test 7 covers each format.
- The Blitz hook exposes `usvg::ImageHrefResolver` (Blitz and fulgur already share usvg).
- In strict mode an SVG problem surfaces after layout, not in `check_images`.
- Non-goals: SVG text and fonts (F152); F177b (usvg reads host files for non-`data:` hrefs: now validated, but not
  offline — resolve through `AssetBundle` or refuse); partial rows of a truncated raster (F32l); WebP for top-level
  `<img>`; `preserveAspectRatio` (F32d).
