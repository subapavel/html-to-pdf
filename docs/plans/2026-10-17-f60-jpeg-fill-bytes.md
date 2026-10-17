# F60 — a JPEG with `0xFF` fill bytes before a marker draws nothing, without a warning

Date: 2026-10-17. Finding: F60 in `2026-10-15-m0-findings.md`. Status: **planned**. Fix on `subapavel/fulgur`
`htmltopdf/srgb-colours` (from `f757f56`).

## 1. Problem

`/tmp/claude-0/pdfrev-f32a/img/fill.jpg` (PIL baseline JPEG, `FF FF FF` inserted before the first DQT, `gen2.py`) in
`single/fill.jpg.html` (`<img src="fill.jpg" style="width:100px">`): Chrome draws it; main and F32a paint nothing and log
nothing. Krilla is **not** the layer: `Image::from_jpeg` reads headers with zune-jpeg **0.5.15** (`jpeg_metadata`,
`krilla-0.7.0/src/graphics/image.rs:583`; the 0.4.21 in the lockfile is resvg's), and 0.5.15 skips fill bytes
(`zune-jpeg-0.5.15/src/decoder.rs:440-458`) leniently and strictly — which is why `validate_detailed` passes the file.
The drop is fulgur's own header scan `ImageRender::decode_dimensions` (`image.rs:300-332`): at `FF FF FF DB` it takes
`0xFF` as the marker, reads `FF FF` as a 65535-byte segment length, jumps past the end and returns `None` (on a larger
file it could land on a random `FF Cx` in the scan data and return a **wrong** size). Then `seed_image_natural_sizes`
(`blitz_adapter.rs:1275`) seeds nothing, `width:100px` gives a 100 × 0 box and `draw_image_inner_paint`
(`render.rs:2833`) skips it; a `background-image` falls back to a 1 × 1 natural size (`convert/style/background.rs:55`);
a `list-style-image` marker is dropped (`list_marker.rs:38`).

## 2. Spec (from memory, wording unverified)

- ITU-T T.81 §B.1.1.2: "Any marker may optionally be preceded by any number of fill bytes, which are bytes assigned code
  X'FF'." Table B.1: SOI, EOI, RSTm and TEM stand alone (no length field).
- ISO 32000-1 §7.4.8: `DCTDecode` data is ISO/IEC 10918-1 (= T.81), so fill bytes are legal in the PDF stream too.
- css-images-3 §5 / HTML §4.8.4.3: a decodable image has natural dimensions and the `<img>` is sized from them.

## 3. Design (fulgur fork, `crates/fulgur/src/image.rs`)

- `ImageRender::decode_dimensions`, JPEG branch: after a `0xFF`, skip further `0xFF` bytes before reading the marker.
  The scan stops at the first SOF, so it never touches entropy-coded data (`FF 00` stuffing, `RSTm`).
- No byte normalisation, no Krilla change: Krilla and zune-jpeg accept fill bytes, and the embedded stream stays the
  file's own bytes (`Repr::Jpeg` passes them through). A file without fill bytes takes exactly the same steps as on
  main, so its size, layout and PDF bytes are unchanged.
- No silent drop (F65-style): in `validate_jpeg`, compare `decode_dimensions` with zune's `dimensions()` (before the
  `MAX_JPEG_VALIDATION_PIXELS` early return; header only). On a mismatch set `damaged` to a reason ("the size in the
  header could not be read (decoder: W × H); it is drawn only where CSS gives both sizes"). Ours already logs `damaged`
  once per `<img>` (`lib.rs:318-320`); strict mode is unchanged. Helper:
  `fn header_size_note(scanned: Option<(u32, u32)>, decoded: (usize, usize)) -> Option<String>`.
- Not chosen: replacing the scan with zune's `decode_headers` (it would change the box of other broken JPEGs; revisit
  if the note fires on a real file).

## 4. Acceptance

`crates/htmltopdf/tests/image_robustness.rs`, with `data/gradient64.jpg` and a helper
`jpeg_with_fill_bytes(markers: &[u8], n: usize) -> Vec<u8>` inserting `n` × `0xFF` before the first occurrence of each
marker in the header (search only before SOS):

1. `jpeg_with_fill_bytes_is_painted_at_its_natural_size`, cases DQT, SOF0, DHT, SOS, all four × 7 bytes: 1 XObject, a
   non-white 64 × 64 ±1 bbox at the content origin, blue marker top equal to the intact JPEG's ±1. **Red** for DQT,
   SOF0, all-four; green guards for DHT, SOS (size read before them).
2. `fill_byte_jpeg_with_css_width_keeps_its_ratio` (`width:100px`): bbox 100 × 100 ±1. **Red.**
3. `fill_byte_jpeg_as_background_image_is_drawn_at_natural_size`: a 64 × 64 `div`, `no-repeat`, covered 64 × 64. **Red.**
4. `image_warnings.rs`: no warning for a fill-byte JPEG; `image_strict.rs`: strict render `Ok`. Guards.

Fork: `jpeg_decode_dimensions_skips_fill_bytes` (synthetic, `FF FF FF` before APP0 and SOF; **red**); `header_size_note`
unit tests; a `render_smoke` test (fill-byte `<img>` → 1 XObject). Fixtures 01–11 byte-identical (`cmp`), compare
numbers unchanged; no new fixture (fill bytes are invisible in HTML; the test generates them).

## 5. Steps

0. Fork: a throwaway test showing `decode_dimensions(fill.jpg) == None` and `krilla_image(fill.jpg).is_some()`.
1. Ours: tests 1–4. Fork: the red `decode_dimensions` test.
2. Fork: the fill-byte skip + `render_smoke`; tests 1–3 green.
3. Fork: `header_size_note` in `validate_jpeg` + unit tests; `cargo test -p fulgur --lib`, `fulgur-vrt` goldens
   byte-identical.
4. Ours: lock bump, `cargo test`, fixtures 01–11 `cmp`, compare, PNGs; `vrt-validator` and `pdf-reviewer` (the fill-byte
   DCT stream must decode in poppler, mutool, `qpdf --check`); `docs/upstream.md`, findings, §7.

## 6. Risks and non-goals

- A PDF consumer whose DCT decoder rejects fill bytes would show nothing (T.81 allows them, libjpeg skips them);
  `pdf-reviewer` checks. Fallback if one fails: strip header fill bytes before Krilla (not done here: every file would
  stop being a pure pass-through).
- The new note can fire on JPEGs the naive scan misreads for other reasons (a standalone TEM / RSTm before SOF):
  warnings only, no pixel change, intended.
- Non-goals: **F61** (no EOI); standalone markers in `decode_dimensions`; EXIF orientation (F32h); JPEGs inside SVG
  (resvg, zune-jpeg 0.4.21); the `url()` strict scan (F65).

## 7. Result

(Filled in at landing.)
