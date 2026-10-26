# ADR-0006: An SVG's image hrefs are resolved by bundle name, never from the filesystem

Date: 2026-10-26 · Status: accepted · First use: F177b (`docs/plans/2026-10-26-f177b-svg-href-offline.md`)

## Context

usvg's default resolver reads every `<image href>` that is not a `data:` URL from the host disk, relative to the
process's working directory or absolute. A document could embed any image file the process can read, the output
depended on where the CLI ran, and `href="/dev/zero"` was an unbounded read. That breaks AGENTS.md goal 3 (only the
input and the bundle decide the output; nothing is read from the host or the network). A top-level `<img src>` has
always been resolved by name among the caller's bundled images (F32a: `ImageProblemKind::Unregistered` when the name
is not bundled). SVG Integration §3 processes an SVG used as an image (`<img>`, CSS `url()` images) in secure mode,
which loads no external resources; `data:` URLs are not external.

## Decision

- An inline `<svg>`: a non-`data:` href is looked up by name among the bundled images with the `<img src>` rule
  (exact key, a leading `./` dropped). Found → decoded and checked like any raster (F177). Not found → the image is
  left out with a warning naming the SVG and the href.
- An SVG used as an image (`<img src>`, `background-image`, `list-style-image`): a non-`data:` href is never loaded,
  bundled or not; left out with a warning. A relative href inside a bundled SVG is therefore never interpreted.
- An inline `<svg>` in a page margin box has no bundle: every non-`data:` href is left out with a warning.
- The filesystem is never read by any SVG parse, including the bundled-image check before the engine runs
  (`usvg::Options` with fs-free resolvers everywhere).
- With `strict_images(true)` a left-out href refuses the document (`ImageProblemKind::UnregisteredInSvg`), as an
  unbundled `<img src>` does (`Unregistered`).

## Consequences

- A document that relied on a host-relative or absolute href loses that image, with a warning — intended.
- Strict mode is stricter than Chrome for an SVG used as an image with a stale external href (Chrome silently omits
  it); strict is opt-in.
- An inline `<svg>` can reference a bundled raster by name, which `data:` URLs made unnecessary before but which is
  how authors write logos.
- Not covered: a fulgur user who sets `base_path` gets SVGs served by `FulgurNetProvider` and parsed by Blitz's
  `net.rs` with usvg's default resolver (F177d; unreachable from our crate, which never sets it).
